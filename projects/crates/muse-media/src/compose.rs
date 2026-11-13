//! Deterministic frame composition from a [`RenderPlan`].
use anyhow::{Context, Result, bail};
use image::{Rgb, RgbImage, imageops};
use media_codec::{DecodeLimits, MediaLibrary};
use media_image::{Bitmap, Frame};
use media_post::{
    EmberPulse, FilmGrain, SoftBloom, Vignette, crossfade, flash, iris_wipe, slices_wipe,
    SlamStamp,
};
use muse_ir::{RenderPlan, ShotTransition};
use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
};

use crate::gpu::{CompositorKind, CompositorPreference, compositor_preference};

/// Decode budget handed to the host codec. The project layer owns the limits;
/// `unbounded` trusts the container's declared size, matching the historical
/// Muse behaviour (no invented budget).
pub(crate) const DECODE_LIMITS: DecodeLimits = DecodeLimits::unbounded();

/// Which compositor actually executed the latest frame, and whether an
/// `auto` preference fell back to CPU. The host must report the real
/// execution path (§4.8), never claim hardware after a silent downgrade.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ComposeReport {
    /// `gpu` | `cpu` | `none` (no frame composed yet).
    pub path: &'static str,
    /// True when `MUSE_COMPOSITOR=auto` fell back from GPU to CPU.
    pub fell_back: bool,
    /// GPU failure recorded at the first fallback.
    pub message: String,
}

#[derive(Default)]
struct ComposePath {
    active: std::sync::atomic::AtomicU8,
    fallback: std::sync::Mutex<Option<String>>,
}

impl ComposePath {
    fn record(&self, kind: CompositorKind, fallback: Option<String>) {
        let code = match kind {
            CompositorKind::Gpu => 2,
            CompositorKind::Cpu if fallback.is_some() => 3,
            CompositorKind::Cpu => 1,
        };
        self.active.store(code, std::sync::atomic::Ordering::Relaxed);
        if let Some(message) = fallback {
            let mut slot = self.fallback.lock().expect("compose path mutex");
            let first = slot.is_none();
            if first {
                *slot = Some(message.clone());
            }
            if first {
                eprintln!("muse: GPU compositor failed ({message}); using CPU compositor");
            }
        }
    }

    fn report(&self) -> ComposeReport {
        match self.active.load(std::sync::atomic::Ordering::Relaxed) {
            1 => ComposeReport { path: "cpu", fell_back: false, message: String::new() },
            2 => ComposeReport { path: "gpu", fell_back: false, message: String::new() },
            3 => ComposeReport {
                path: "cpu",
                fell_back: true,
                message: self
                    .fallback
                    .lock()
                    .expect("compose path mutex")
                    .clone()
                    .unwrap_or_default(),
            },
            _ => ComposeReport { path: "none", fell_back: false, message: String::new() },
        }
    }
}

static COMPOSE_PATH: ComposePath = ComposePath {
    active: std::sync::atomic::AtomicU8::new(0),
    fallback: std::sync::Mutex::new(None),
};

/// Execution path of the latest composed frame for host reporting.
#[must_use]
pub fn compose_report() -> ComposeReport {
    COMPOSE_PATH.report()
}

#[derive(Debug, Clone)]
pub struct ComposeOptions {
    pub asset_root: PathBuf,
}

impl Default for ComposeOptions {
    fn default() -> Self {
        Self {
            asset_root: PathBuf::from("."),
        }
    }
}

/// Resolve the first declared solid background color from the plan, defaulting to near-black.
pub fn plan_background_rgb(plan: &RenderPlan) -> (u8, u8, u8, Option<String>) {
    let color = plan
        .visuals
        .iter()
        .find_map(|layer| layer.background_color.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty());
    match color {
        Some(raw) => match parse_hex_color(raw) {
            Some(rgb) => (rgb.0, rgb.1, rgb.2, Some(raw.to_owned())),
            None => (11, 21, 36, Some(raw.to_owned())),
        },
        None => (11, 21, 36, None),
    }
}

fn parse_hex_color(raw: &str) -> Option<(u8, u8, u8)> {
    let hex = raw.strip_prefix('#').unwrap_or(raw);
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some((r, g, b))
}

/// Paint one frame at normalized time `t` in `[0, 1]` across the plan duration.
///
/// Returns an RGBA8 [`Frame`] from the shared image storage; `media` is the
/// per-render decode cache (host codec import with [`DECODE_LIMITS`]).
/// Prefers the GPU compositor when available (`MUSE_COMPOSITOR=auto|gpu`).
/// Forced GPU (`MUSE_COMPOSITOR=gpu`) fails explicitly if the backend cannot init.
/// Incoming shot `xfade` / `flash` / `iris` / `slices` blend previous and current looks when active.
pub fn compose_frame(
    plan: &RenderPlan,
    width: u32,
    height: u32,
    t: f32,
    options: &ComposeOptions,
    media: &mut MediaLibrary,
) -> Result<Frame> {
    Ok(frame_from_rgb(&compose_frame_rgb(
        plan, width, height, t, options, media,
    )?))
}

fn compose_frame_rgb(
    plan: &RenderPlan,
    width: u32,
    height: u32,
    t: f32,
    options: &ComposeOptions,
    media: &mut MediaLibrary,
) -> Result<RgbImage> {
    if let Some(tr) = plan.transition_at_normalized(t) {
        let from = compose_frame_plain(plan, width, height, tr.from_t, options, media)?;
        let to = compose_frame_plain(plan, width, height, t, options, media)?;
        // 转场混合算子归 `media.post`（显示空间，逐值移植自本模块）。
        let (from_frame, to_frame) = (frame_from_rgb(&from), frame_from_rgb(&to));
        let mut blended = match tr.kind {
            ShotTransition::Iris => iris_wipe(&from_frame, &to_frame, tr.mix),
            ShotTransition::Slices => slices_wipe(&from_frame, &to_frame, tr.mix),
            _ => crossfade(&from_frame, &to_frame, tr.mix),
        };
        if tr.flash > 0.0 {
            flash(&mut blended, tr.flash);
        }
        return Ok(rgb_from_frame(&blended));
    }
    compose_frame_plain(plan, width, height, t, options, media)
}

/// Wrap an RGB canvas into shared RGBA8 storage with full opacity.
fn frame_from_rgb(image: &RgbImage) -> Frame {
    let mut frame = Frame::new(image.width(), image.height());
    for (index, pixel) in image.pixels().enumerate() {
        let base = index * 4;
        frame.pixels[base] = pixel[0];
        frame.pixels[base + 1] = pixel[1];
        frame.pixels[base + 2] = pixel[2];
        frame.pixels[base + 3] = 255;
    }
    frame
}

/// Drop the alpha channel of a shared [`Frame`] back to the RGB canvas that
/// host-side painting (glyphs, progress cue) still draws into.
fn rgb_from_frame(frame: &Frame) -> RgbImage {
    RgbImage::from_fn(frame.width, frame.height, |x, y| {
        let base = ((y * frame.width + x) * 4) as usize;
        Rgb([
            frame.pixels[base],
            frame.pixels[base + 1],
            frame.pixels[base + 2],
        ])
    })
}

/// Drop the alpha channel of a decoded [`Bitmap`] for the RGB operator pipeline.
fn rgb_from_bitmap(bitmap: &Bitmap) -> RgbImage {
    let mut out = RgbImage::new(bitmap.width, bitmap.height);
    for y in 0..bitmap.height {
        for x in 0..bitmap.width {
            let base = ((y * bitmap.width + x) as usize) * 4;
            out.put_pixel(
                x,
                y,
                Rgb([bitmap.rgba[base], bitmap.rgba[base + 1], bitmap.rgba[base + 2]]),
            );
        }
    }
    out
}

fn compose_frame_plain(
    plan: &RenderPlan,
    width: u32,
    height: u32,
    t: f32,
    options: &ComposeOptions,
    media: &mut MediaLibrary,
) -> Result<RgbImage> {
    match compositor_preference() {
        CompositorPreference::Cpu => {
            COMPOSE_PATH.record(CompositorKind::Cpu, None);
            compose_frame_cpu(plan, width, height, t, options, media)
        }
        CompositorPreference::Gpu => {
            let image = compose_frame_gpu(plan, width, height, t, options, media)
                .context("GPU compositor required (MUSE_COMPOSITOR=gpu)")?;
            COMPOSE_PATH.record(CompositorKind::Gpu, None);
            Ok(image)
        }
        CompositorPreference::Auto => {
            match compose_frame_gpu(plan, width, height, t, options, media) {
                Ok(image) => {
                    COMPOSE_PATH.record(CompositorKind::Gpu, None);
                    Ok(image)
                }
                Err(error) => {
                    COMPOSE_PATH.record(CompositorKind::Cpu, Some(error.to_string()));
                    compose_frame_cpu(plan, width, height, t, options, media)
                }
            }
        }
    }
}

fn compose_frame_cpu(
    plan: &RenderPlan,
    width: u32,
    height: u32,
    t: f32,
    options: &ComposeOptions,
    media: &mut MediaLibrary,
) -> Result<RgbImage> {
    let layers = plan.visuals_at_normalized(t);
    let (r, g, b) = layers_background_rgb(&layers);
    let mut canvas = RgbImage::from_pixel(width, height, Rgb([r, g, b]));
    let images: Vec<&str> = layers
        .iter()
        .flat_map(|layer| layer.images.iter().map(String::as_str))
        .collect();
    for uri in images {
        let plate = load_image(uri, options, media)?;
        blit_cover(&mut canvas, &plate);
    }
    // 显示空间算子链：语义归 `media.post::shot_ops`（G7-W2 逐值上移），
    // Muse 只做编排与宿主绘制（字形、进度线）的穿插。
    let progress = plan.shot_progress_normalized(t);
    let mut frame = frame_from_rgb(&canvas);
    if let Some((name, mix)) = layers_lut(&layers) {
        let grade = crate::lut::resolve_lut(&name, &options.asset_root)?;
        grade.apply(&mut frame, mix);
    }
    if let Some(strength) = layers_pulse(&layers) {
        EmberPulse::new(strength).apply(&mut frame, progress);
    }
    if let Some((strength, label)) = layers_stamp(&layers) {
        SlamStamp::new(strength).apply(&mut frame, progress);
        if let Some(label) = label.as_deref() {
            canvas = rgb_from_frame(&frame);
            crate::font_stamp::draw_stamp_label(&mut canvas, label, strength, progress);
            frame = frame_from_rgb(&canvas);
        }
    }
    if let Some(strength) = layers_bloom(&layers) {
        SoftBloom::new(strength).apply(&mut frame);
    }
    canvas = rgb_from_frame(&frame);
    // Lyrics default to post_group none — draw after bloom (doc 14).
    paint_lyric_layers(&mut canvas, plan, t);
    let mut frame = frame_from_rgb(&canvas);
    if let Some(strength) = layers_vignette(&layers) {
        Vignette::new(strength).apply(&mut frame);
    }
    if let Some(strength) = layers_grain(&layers) {
        FilmGrain::new(strength).apply(&mut frame, t);
    }
    canvas = rgb_from_frame(&frame);
    paint_progress_cue(&mut canvas, t);
    Ok(canvas)
}

#[cfg(feature = "gpu")]
fn compose_frame_gpu(
    plan: &RenderPlan,
    width: u32,
    height: u32,
    t: f32,
    options: &ComposeOptions,
    media: &mut MediaLibrary,
) -> Result<RgbImage> {
    use crate::gpu::{GpuComposeOptions, GpuCompositor};

    static GPU: OnceLock<Result<GpuCompositor, String>> = OnceLock::new();
    let compositor = match GPU.get_or_init(|| GpuCompositor::new().map_err(|e| e.to_string())) {
        Ok(gpu) => gpu,
        Err(message) => bail!("{message}"),
    };

    let layers: Vec<_> = plan
        .visuals_at_normalized(t)
        .into_iter()
        .cloned()
        .collect();
    let rgba = compositor.compose_layers(
        &layers,
        &GpuComposeOptions {
            width,
            height,
            asset_root: options.asset_root.clone(),
            time: t,
            shot_progress: plan.shot_progress_normalized(t),
        },
        media,
    )?;
    let mut canvas = RgbImage::from_fn(width, height, |x, y| {
        let p = rgba.get_pixel(x, y);
        Rgb([p[0], p[1], p[2]])
    });
    // Geometry stamp stays on GPU; glyph label is CPU-rasterized over the frame.
    let progress = plan.shot_progress_normalized(t);
    let layer_refs: Vec<_> = layers.iter().collect();
    if let Some((strength, label)) = layers_stamp(&layer_refs) {
        if let Some(label) = label.as_deref() {
            crate::font_stamp::draw_stamp_label(&mut canvas, label, strength, progress);
        }
    }
    paint_lyric_layers(&mut canvas, plan, t);
    paint_progress_cue(&mut canvas, t);
    Ok(canvas)
}

#[cfg(not(feature = "gpu"))]
fn compose_frame_gpu(
    _plan: &RenderPlan,
    _width: u32,
    _height: u32,
    _t: f32,
    _options: &ComposeOptions,
    _media: &mut MediaLibrary,
) -> Result<RgbImage> {
    bail!("muse-media built without `gpu` feature")
}

fn paint_progress_cue(canvas: &mut RgbImage, t: f32) {
    let width = canvas.width();
    let height = canvas.height();
    let progress_x =
        ((t.clamp(0.0, 1.0) * (width.saturating_sub(1) as f32)) as u32).min(width.saturating_sub(1));
    for y in (height.saturating_sub(4))..height {
        canvas.put_pixel(progress_x, y, Rgb([220, 220, 220]));
    }
}

fn load_image(
    uri: &str,
    options: &ComposeOptions,
    media: &mut MediaLibrary,
) -> Result<RgbImage> {
    let path = resolve_asset(uri, &options.asset_root);
    let bitmap = media
        .image(&path.to_string_lossy(), DECODE_LIMITS)
        .with_context(|| format!("cannot open visual image `{}` ({})", uri, path.display()))?;
    Ok(rgb_from_bitmap(&bitmap))
}

fn resolve_asset(uri: &str, root: &Path) -> PathBuf {
    let path = Path::new(uri);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

/// Last declared vignette strength on the plan, if any.
pub fn plan_vignette(plan: &RenderPlan) -> Option<f32> {
    layers_vignette(&plan.visuals.iter().collect::<Vec<_>>())
}

/// Last declared grain strength on the plan, if any.
pub fn plan_grain(plan: &RenderPlan) -> Option<f32> {
    layers_grain(&plan.visuals.iter().collect::<Vec<_>>())
}

/// Last declared bloom strength on the plan, if any.
pub fn plan_bloom(plan: &RenderPlan) -> Option<f32> {
    layers_bloom(&plan.visuals.iter().collect::<Vec<_>>())
}

/// Last declared LUT name + mix on the plan, if any.
pub fn plan_lut(plan: &RenderPlan) -> Option<(String, f32)> {
    layers_lut(&plan.visuals.iter().collect::<Vec<_>>())
}

/// First `LyricCard` prefab instance ports (doc 20): `cue` + `intensity`.
fn lyric_card_ports(plan: &RenderPlan) -> Option<(Option<&str>, f32)> {
    let instance = plan
        .prefab_instances
        .iter()
        .find(|inst| inst.prefab == "LyricCard")?;
    let cue = instance
        .ports
        .get("cue")
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());
    let intensity = instance
        .ports
        .get("intensity")
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(1.0)
        .clamp(0.0, 2.0);
    Some((cue, intensity))
}

/// Caption opacity from LyricCard: `intensity` × optional non-focus dim (0.35).
fn lyric_card_opacity(plan: &RenderPlan, line_cue: Option<&str>) -> f32 {
    let Some((focus_cue, intensity)) = lyric_card_ports(plan) else {
        return 0.95;
    };
    let mut opacity = 0.95 * intensity;
    if let Some(focus) = focus_cue {
        if line_cue != Some(focus) {
            opacity *= 0.35;
        }
    }
    opacity.clamp(0.0, 1.0)
}

fn paint_lyric_layers(canvas: &mut RgbImage, plan: &RenderPlan, t_norm: f32) {
    if plan.lyric_layers.is_empty() || plan.lyric_tracks.is_empty() {
        return;
    }
    let duration = plan.duration_seconds;
    if duration <= 0.0 {
        return;
    }
    let t_sec = f64::from(t_norm.clamp(0.0, 1.0)) * duration;
    for layer in &plan.lyric_layers {
        let Some(track) = plan.lyric_tracks.iter().find(|tr| tr.id == layer.track) else {
            continue;
        };
        let Some(line) = track.active_line_at(t_sec) else {
            continue;
        };
        let active = track.active_word_at(t_sec);
        let opacity = lyric_card_opacity(plan, line.cue_id.as_deref());
        if track.uses_localized_display(line) {
            let display = track
                .localized_text_for_line(line)
                .map(|t| t.text.as_str());
            crate::font_stamp::draw_lyric_caption_with_display(
                canvas, layer, line, active, display, opacity,
            );
        } else {
            crate::font_stamp::draw_lyric_caption_with_display(
                canvas, layer, line, active, None, opacity,
            );
        }
    }
}

fn layers_background_rgb(layers: &[&muse_ir::VisualLayer]) -> (u8, u8, u8) {
    let color = layers
        .iter()
        .rev()
        .find_map(|layer| layer.background_color.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty());
    match color {
        Some(raw) => parse_hex_color(raw).unwrap_or((11, 21, 36)),
        None => (11, 21, 36),
    }
}

fn layers_vignette(layers: &[&muse_ir::VisualLayer]) -> Option<f32> {
    layers.iter().rev().find_map(|layer| layer.vignette)
}

fn layers_grain(layers: &[&muse_ir::VisualLayer]) -> Option<f32> {
    layers.iter().rev().find_map(|layer| layer.grain)
}

fn layers_pulse(layers: &[&muse_ir::VisualLayer]) -> Option<f32> {
    layers.iter().rev().find_map(|layer| layer.pulse)
}

fn layers_stamp(layers: &[&muse_ir::VisualLayer]) -> Option<(f32, Option<String>)> {
    layers
        .iter()
        .rev()
        .find_map(|layer| layer.stamp.map(|strength| (strength, layer.stamp_label.clone())))
}

fn layers_bloom(layers: &[&muse_ir::VisualLayer]) -> Option<f32> {
    layers.iter().rev().find_map(|layer| layer.bloom)
}

fn layers_lut(layers: &[&muse_ir::VisualLayer]) -> Option<(String, f32)> {
    layers.iter().rev().find_map(|layer| {
        let name = layer.lut.as_ref()?;
        let mix = layer.lut_mix.unwrap_or(1.0);
        Some((name.clone(), mix))
    })
}

/// Cover-fit `src` onto `dst` (center-cropped), matching Unity/CSS `object-fit: cover`.
fn blit_cover(dst: &mut RgbImage, src: &RgbImage) {
    let (dw, dh) = (dst.width(), dst.height());
    let (sw, sh) = (src.width(), src.height());
    if sw == 0 || sh == 0 || dw == 0 || dh == 0 {
        return;
    }
    let scale = f32::max(dw as f32 / sw as f32, dh as f32 / sh as f32);
    let tw = ((sw as f32 * scale).round() as u32).max(1);
    let th = ((sh as f32 * scale).round() as u32).max(1);
    let resized = imageops::resize(src, tw, th, imageops::FilterType::Triangle);
    let x0 = tw.saturating_sub(dw) / 2;
    let y0 = th.saturating_sub(dh) / 2;
    let cropped = imageops::crop_imm(&resized, x0, y0, dw, dh).to_image();
    imageops::replace(dst, &cropped, 0, 0);
}

pub fn plan_has_images(plan: &RenderPlan) -> bool {
    plan.visuals.iter().any(|layer| !layer.images.is_empty())
}

#[cfg(test)]
mod tests {
    use super::{
        ComposeOptions, ComposePath, compose_frame, lyric_card_opacity, lyric_card_ports,
    };
    use crate::gpu::CompositorKind;

    #[test]
    fn compose_path_reports_real_execution_and_fallback() {
        let path = ComposePath::default();
        assert_eq!(path.report().path, "none");
        path.record(CompositorKind::Cpu, None);
        let report = path.report();
        assert_eq!((report.path, report.fell_back, report.message.as_str()), ("cpu", false, ""));
        path.record(CompositorKind::Gpu, None);
        let report = path.report();
        assert_eq!((report.path, report.fell_back), ("gpu", false));
        path.record(CompositorKind::Cpu, Some("no adapter".into()));
        let report = path.report();
        assert_eq!(
            (report.path, report.fell_back, report.message.as_str()),
            ("cpu", true, "no adapter"),
            "auto fallback must be reported as CPU execution with the GPU failure"
        );
    }
    use image::{Rgb, RgbImage};
    use media_codec::MediaLibrary;
    use muse_ir::{
        AudioSourceKind, PrefabInstance, RenderPlan, TimeMap,
    };
    use muse_types::RenderSettings;
    use std::collections::BTreeMap;

    fn fnv1a64(bytes: &[u8]) -> u64 {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in bytes {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        hash
    }

    /// G7-W2 迁移门 golden：CPU 通路全显示算子（lut/pulse/stamp/bloom/
    /// vignette/grain + 进度条）单帧整像素 digest。算子逐值上移到
    /// `media.post` 后本 digest 必须不变（逐值移植、无行为修正）。
    #[test]
    fn golden_cpu_frame_all_display_ops() {
        unsafe {
            std::env::set_var("MUSE_COMPOSITOR", "cpu");
        }
        let plan = muse_ir::compile(
            r##"timeline { act main : 0..1 } visual for main { background color "#c05030" vignette 0.6 grain 0.25 bloom 0.5 lut P2 0.4 fx pulse 0.7 fx stamp 0.8 }"##,
            Default::default(),
        )
        .plan
        .expect("valid plan");
        let mut media = MediaLibrary::new();
        let frame =
            compose_frame(&plan, 64, 48, 0.5, &ComposeOptions::default(), &mut media).unwrap();
        assert_eq!(fnv1a64(&frame.pixels), 7651537512841525761);
        unsafe {
            std::env::remove_var("MUSE_COMPOSITOR");
        }
    }

    #[test]
    fn composes_background_frame_with_full_alpha_cpu() {
        // Exact RGB asserts use the CPU path (GPU sRGB clear can round differently).
        unsafe {
            std::env::set_var("MUSE_COMPOSITOR", "cpu");
        }
        let plan = muse_ir::compile(
            r##"timeline { act main : 0..1 } visual for main { background color "#0b1524" }"##,
            Default::default(),
        )
        .plan
        .expect("valid plan");
        let mut media = MediaLibrary::new();
        let frame =
            compose_frame(&plan, 32, 24, 0.0, &ComposeOptions::default(), &mut media).unwrap();
        assert_eq!(&frame.pixels[0..4], &[0x0b, 0x15, 0x24, 255]);
        // PNG round-trip through the shared codec decodes back to the same pixels.
        let dir = std::env::temp_dir().join(format!("muse-media-frame-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let png = dir.join("frame.png");
        media_codec::write_png(&png, frame.width, frame.height, &frame.pixels).unwrap();
        let decoded = image::open(&png).unwrap().to_rgb8();
        assert_eq!(*decoded.get_pixel(0, 0), Rgb([0x0b, 0x15, 0x24]));
        let _ = std::fs::remove_dir_all(&dir);
        unsafe {
            std::env::remove_var("MUSE_COMPOSITOR");
        }
    }

    #[cfg(windows)]
    #[test]
    fn composes_image_layer_and_rejects_missing_asset_explicitly() {
        unsafe {
            std::env::set_var("MUSE_COMPOSITOR", "cpu");
        }
        let dir = std::env::temp_dir().join(format!("muse-media-image-{}", std::process::id()));
        let textures = dir.join("assets/textures");
        std::fs::create_dir_all(&textures).unwrap();
        let mut plate = RgbImage::new(32, 32);
        for pixel in plate.pixels_mut() {
            *pixel = Rgb([255, 0, 0]);
        }
        plate.save(textures.join("plate.png")).unwrap();

        let plan = muse_ir::compile(
            r##"timeline { act main : 0..1 } visual for main { background color "#0b1524" image "assets/textures/plate.png" }"##,
            Default::default(),
        )
        .plan
        .expect("valid plan");
        let options = ComposeOptions {
            asset_root: dir.clone(),
        };
        let mut media = MediaLibrary::new();
        let frame = compose_frame(&plan, 16, 16, 0.0, &options, &mut media).unwrap();
        assert_eq!(&frame.pixels[0..4], &[255, 0, 0, 255]);

        // Missing asset: explicit error naming the uri and the resolved path.
        let missing = muse_ir::compile(
            r##"timeline { act main : 0..1 } visual for main { image "assets/textures/absent.png" }"##,
            Default::default(),
        )
        .plan
        .expect("valid plan");
        let error = match compose_frame(&missing, 16, 16, 0.0, &options, &mut MediaLibrary::new()) {
            Ok(_) => panic!("missing asset must fail"),
            Err(error) => error,
        };
        let message = error.to_string();
        assert!(message.contains("cannot open visual image"), "{message}");
        assert!(message.contains("assets/textures/absent.png"), "{message}");
        let _ = std::fs::remove_dir_all(&dir);
        unsafe {
            std::env::remove_var("MUSE_COMPOSITOR");
        }
    }

    fn empty_plan(instances: Vec<PrefabInstance>) -> RenderPlan {
        RenderPlan {
            settings: RenderSettings::default(),
            duration_seconds: 1.0,
            time_map: TimeMap::new(120.0, 4),
            acts: vec![],
            shots: vec![],
            assets: vec![],
            audio_source: AudioSourceKind::PlaceholderTone,
            audio_tracks: vec![],
            lyric_tracks: vec![],
            lyric_layers: vec![],
            visual_layers: 0,
            visuals: vec![],
            transitions: 0,
            intents: vec![],
            orchestration: vec![],
            prefab_instances: instances,
        }
    }

    #[test]
    fn lyric_card_intensity_and_cue_focus_dim() {
        assert!((lyric_card_opacity(&empty_plan(vec![]), Some("C09")) - 0.95).abs() < 1e-6);

        let mut ports = BTreeMap::new();
        ports.insert("cue".into(), "C09".into());
        ports.insert("intensity".into(), "1.0".into());
        let plan = empty_plan(vec![PrefabInstance {
            id: "caption".into(),
            prefab: "LyricCard".into(),
            ports,
        }]);
        assert_eq!(lyric_card_ports(&plan).unwrap().0, Some("C09"));
        assert!((lyric_card_opacity(&plan, Some("C09")) - 0.95).abs() < 1e-6);
        assert!((lyric_card_opacity(&plan, Some("C01")) - 0.95 * 0.35).abs() < 1e-6);

        let mut dim = BTreeMap::new();
        dim.insert("intensity".into(), "0.5".into());
        let dim_plan = empty_plan(vec![PrefabInstance {
            id: "caption".into(),
            prefab: "LyricCard".into(),
            ports: dim,
        }]);
        assert!((lyric_card_opacity(&dim_plan, None) - 0.95 * 0.5).abs() < 1e-6);
    }
}
