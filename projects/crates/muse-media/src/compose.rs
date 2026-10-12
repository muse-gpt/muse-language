//! Deterministic frame composition from a [`RenderPlan`].
use anyhow::{Context, Result, bail};
use image::{Rgb, RgbImage, imageops};
use muse_ir::{RenderPlan, ShotTransition};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::OnceLock,
};

use crate::gpu::{CompositorPreference, compositor_preference};

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
/// Prefers the GPU compositor when available (`MUSE_COMPOSITOR=auto|gpu`).
/// Forced GPU (`MUSE_COMPOSITOR=gpu`) fails explicitly if the backend cannot init.
/// Incoming shot `xfade` / `flash` / `iris` blend previous and current looks when active.
pub fn compose_frame(
    plan: &RenderPlan,
    width: u32,
    height: u32,
    t: f32,
    options: &ComposeOptions,
    cache: &mut HashMap<String, RgbImage>,
) -> Result<RgbImage> {
    if let Some(tr) = plan.transition_at_normalized(t) {
        let from = compose_frame_plain(plan, width, height, tr.from_t, options, cache)?;
        let to = compose_frame_plain(plan, width, height, t, options, cache)?;
        let mut blended = match tr.kind {
            ShotTransition::Iris => iris_blend_rgb(&from, &to, tr.mix),
            _ => lerp_rgb(&from, &to, tr.mix),
        };
        if tr.flash > 0.0 {
            apply_flash_rgb(&mut blended, tr.flash);
        }
        return Ok(blended);
    }
    compose_frame_plain(plan, width, height, t, options, cache)
}

fn compose_frame_plain(
    plan: &RenderPlan,
    width: u32,
    height: u32,
    t: f32,
    options: &ComposeOptions,
    cache: &mut HashMap<String, RgbImage>,
) -> Result<RgbImage> {
    match compositor_preference() {
        CompositorPreference::Cpu => compose_frame_cpu(plan, width, height, t, options, cache),
        CompositorPreference::Gpu => {
            compose_frame_gpu(plan, width, height, t, options)
                .context("GPU compositor required (MUSE_COMPOSITOR=gpu)")
        }
        CompositorPreference::Auto => match compose_frame_gpu(plan, width, height, t, options) {
            Ok(image) => Ok(image),
            Err(_) => compose_frame_cpu(plan, width, height, t, options, cache),
        },
    }
}

fn lerp_rgb(a: &RgbImage, b: &RgbImage, mix: f32) -> RgbImage {
    let mix = mix.clamp(0.0, 1.0);
    let width = a.width().min(b.width());
    let height = a.height().min(b.height());
    RgbImage::from_fn(width, height, |x, y| {
        let pa = a.get_pixel(x, y);
        let pb = b.get_pixel(x, y);
        Rgb([
            ((pa[0] as f32 * (1.0 - mix) + pb[0] as f32 * mix).round() as u8),
            ((pa[1] as f32 * (1.0 - mix) + pb[1] as f32 * mix).round() as u8),
            ((pa[2] as f32 * (1.0 - mix) + pb[2] as f32 * mix).round() as u8),
        ])
    })
}

/// Circular iris wipe: current look opens from center over previous look.
fn iris_blend_rgb(from: &RgbImage, to: &RgbImage, mix: f32) -> RgbImage {
    let mix = mix.clamp(0.0, 1.0);
    let width = from.width().min(to.width());
    let height = from.height().min(to.height());
    let cx = width as f32 * 0.5;
    let cy = height as f32 * 0.5;
    let max_r = (cx * cx + cy * cy).sqrt().max(1.0);
    let open = max_r * (mix * mix * (3.0 - 2.0 * mix));
    let feather = (max_r * 0.05).max(1.5);
    RgbImage::from_fn(width, height, |x, y| {
        let dx = x as f32 + 0.5 - cx;
        let dy = y as f32 + 0.5 - cy;
        let d = (dx * dx + dy * dy).sqrt();
        let t = (1.0
            - ((d - (open - feather)) / (2.0 * feather)).clamp(0.0, 1.0))
            .clamp(0.0, 1.0);
        let mask = t * t * (3.0 - 2.0 * t);
        let pa = from.get_pixel(x, y);
        let pb = to.get_pixel(x, y);
        Rgb([
            ((pa[0] as f32 * (1.0 - mask) + pb[0] as f32 * mask).round() as u8),
            ((pa[1] as f32 * (1.0 - mask) + pb[1] as f32 * mask).round() as u8),
            ((pa[2] as f32 * (1.0 - mask) + pb[2] as f32 * mask).round() as u8),
        ])
    })
}

fn apply_flash_rgb(canvas: &mut RgbImage, amount: f32) {
    let amount = amount.clamp(0.0, 1.0);
    if amount <= 0.0 {
        return;
    }
    for pixel in canvas.pixels_mut() {
        for c in 0..3 {
            pixel[c] = ((pixel[c] as f32 * (1.0 - amount) + 255.0 * amount).round() as u8).min(255);
        }
    }
}

fn compose_frame_cpu(
    plan: &RenderPlan,
    width: u32,
    height: u32,
    t: f32,
    options: &ComposeOptions,
    cache: &mut HashMap<String, RgbImage>,
) -> Result<RgbImage> {
    let layers = plan.visuals_at_normalized(t);
    let (r, g, b) = layers_background_rgb(&layers);
    let mut canvas = RgbImage::from_pixel(width, height, Rgb([r, g, b]));
    let images: Vec<&str> = layers
        .iter()
        .flat_map(|layer| layer.images.iter().map(String::as_str))
        .collect();
    for uri in images {
        let plate = load_image(uri, options, cache)?;
        blit_cover(&mut canvas, &plate);
    }
    if let Some((name, mix)) = layers_lut(&layers) {
        let lut = crate::lut::resolve_lut(&name, &options.asset_root)?;
        crate::lut::apply_lut_rgb(&mut canvas, &lut, mix);
    }
    if let Some(strength) = layers_pulse(&layers) {
        apply_pulse_rgb(&mut canvas, strength, plan.shot_progress_normalized(t));
    }
    let progress = plan.shot_progress_normalized(t);
    if let Some((strength, label)) = layers_stamp(&layers) {
        apply_stamp_rgb(&mut canvas, strength, progress);
        if let Some(label) = label.as_deref() {
            crate::font_stamp::draw_stamp_label(&mut canvas, label, strength, progress);
        }
    }
    if let Some(strength) = layers_bloom(&layers) {
        apply_bloom_rgb(&mut canvas, strength);
    }
    if let Some(strength) = layers_vignette(&layers) {
        apply_vignette_rgb(&mut canvas, strength);
    }
    if let Some(strength) = layers_grain(&layers) {
        apply_grain_rgb(&mut canvas, strength, t);
    }
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
    cache: &mut HashMap<String, RgbImage>,
) -> Result<RgbImage> {
    if let Some(hit) = cache.get(uri) {
        return Ok(hit.clone());
    }
    let path = resolve_asset(uri, &options.asset_root);
    let image = image::open(&path)
        .with_context(|| format!("cannot open visual image `{}` ({})", uri, path.display()))?
        .to_rgb8();
    cache.insert(uri.to_owned(), image.clone());
    Ok(image)
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

/// Center ember pulse driven by shot-local progress (shot FX, not lyrics).
fn apply_pulse_rgb(canvas: &mut RgbImage, strength: f32, shot_progress: f32) {
    let strength = strength.clamp(0.0, 1.0);
    if strength <= 0.0 {
        return;
    }
    let envelope = (shot_progress * std::f32::consts::PI).sin().max(0.0);
    let amp = strength * envelope;
    if amp <= 0.0 {
        return;
    }
    let width = canvas.width();
    let height = canvas.height();
    for y in 0..height {
        for x in 0..width {
            let u = (x as f32 + 0.5) / width as f32;
            let v = (y as f32 + 0.5) / height as f32;
            let r2 = (u - 0.5).powi(2) + (v - 0.5).powi(2);
            let glow = (-r2 * 14.0).exp() * amp;
            let p = canvas.get_pixel_mut(x, y);
            p[0] = ((p[0] as f32 / 255.0 + glow * 1.0).clamp(0.0, 1.0) * 255.0).round() as u8;
            p[1] = ((p[1] as f32 / 255.0 + glow * 0.72).clamp(0.0, 1.0) * 255.0).round() as u8;
            p[2] = ((p[2] as f32 / 255.0 + glow * 0.35).clamp(0.0, 1.0) * 255.0).round() as u8;
        }
    }
}

/// Geometric stamp/slam block under the glyph label (shot FX, not a lyric layer).
fn apply_stamp_rgb(canvas: &mut RgbImage, strength: f32, shot_progress: f32) {
    let strength = strength.clamp(0.0, 1.0);
    if strength <= 0.0 {
        return;
    }
    let slam = smoothstep(0.0, 0.22, shot_progress);
    let scale = 2.4 - 1.4 * slam;
    let opacity = strength * slam * (0.75 + 0.25 * (shot_progress * std::f32::consts::PI).sin().abs());
    if opacity <= 0.0 {
        return;
    }
    let width = canvas.width();
    let height = canvas.height();
    let aspect = width as f32 / height as f32;
    for y in 0..height {
        for x in 0..width {
            let u = ((x as f32 + 0.5) / width as f32 - 0.5) * aspect;
            let v = (y as f32 + 0.5) / height as f32 - 0.5;
            let px = u * scale;
            let py = v * scale;
            let d = px.abs().max(py.abs());
            let outer = smoothstep(0.22, 0.18, d) * smoothstep(0.12, 0.14, d);
            let inner = smoothstep(0.10, 0.06, d);
            let mask = (outer * 0.85 + inner * 0.55).clamp(0.0, 1.0) * opacity;
            if mask <= 0.0 {
                continue;
            }
            let p = canvas.get_pixel_mut(x, y);
            p[0] = ((p[0] as f32 / 255.0 * (1.0 - mask) + 0.98 * mask).clamp(0.0, 1.0) * 255.0)
                .round() as u8;
            p[1] = ((p[1] as f32 / 255.0 * (1.0 - mask) + 0.82 * mask).clamp(0.0, 1.0) * 255.0)
                .round() as u8;
            p[2] = ((p[2] as f32 / 255.0 * (1.0 - mask) + 0.55 * mask).clamp(0.0, 1.0) * 255.0)
                .round() as u8;
        }
    }
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Soft highlight bloom matching the GPU 5×5 threshold blur.
fn apply_bloom_rgb(canvas: &mut RgbImage, strength: f32) {
    let strength = strength.clamp(0.0, 1.0);
    if strength <= 0.0 {
        return;
    }
    let width = canvas.width();
    let height = canvas.height();
    let src = canvas.clone();
    let threshold = 0.62_f32;
    for y in 0..height {
        for x in 0..width {
            let mut acc = [0.0_f32; 3];
            for oy in -2i32..=2 {
                for ox in -2i32..=2 {
                    let sx = (x as i32 + ox * 2).clamp(0, width as i32 - 1) as u32;
                    let sy = (y as i32 + oy * 2).clamp(0, height as i32 - 1) as u32;
                    let p = src.get_pixel(sx, sy);
                    for c in 0..3 {
                        let v = p[c] as f32 / 255.0;
                        acc[c] += (v - threshold).max(0.0);
                    }
                }
            }
            let p = canvas.get_pixel_mut(x, y);
            for c in 0..3 {
                let add = (acc[c] / 25.0) * strength * 2.5;
                let v = (p[c] as f32 / 255.0 + add).clamp(0.0, 1.0);
                p[c] = (v * 255.0).round() as u8;
            }
        }
    }
}

/// Radial edge darken matching the GPU vignette pass (`smoothstep(0.25, 0.95, d)`).
fn apply_vignette_rgb(canvas: &mut RgbImage, strength: f32) {
    let strength = strength.clamp(0.0, 1.0);
    if strength <= 0.0 {
        return;
    }
    let width = canvas.width();
    let height = canvas.height();
    for y in 0..height {
        for x in 0..width {
            let u = (x as f32 + 0.5) / width as f32;
            let v = (y as f32 + 0.5) / height as f32;
            let d = ((u - 0.5).hypot(v - 0.5)) * std::f32::consts::SQRT_2;
            let t = ((d - 0.25) / 0.70).clamp(0.0, 1.0);
            let edge = t * t * (3.0 - 2.0 * t);
            let factor = 1.0 - strength * edge;
            let p = canvas.get_pixel_mut(x, y);
            p[0] = (p[0] as f32 * factor).round() as u8;
            p[1] = (p[1] as f32 * factor).round() as u8;
            p[2] = (p[2] as f32 * factor).round() as u8;
        }
    }
}

/// Deterministic film grain matching the GPU post pass.
fn apply_grain_rgb(canvas: &mut RgbImage, strength: f32, time: f32) {
    let strength = strength.clamp(0.0, 1.0);
    if strength <= 0.0 {
        return;
    }
    let time = time.clamp(0.0, 1.0);
    let width = canvas.width();
    let height = canvas.height();
    for y in 0..height {
        for x in 0..width {
            let u = (x as f32 + 0.5) / width as f32;
            let v = (y as f32 + 0.5) / height as f32;
            let n = hash21(u * 1024.0 + time * 97.0, v * 1024.0 + time * 53.0) * 2.0 - 1.0;
            let p = canvas.get_pixel_mut(x, y);
            for c in 0..3 {
                let v = (p[c] as f32 / 255.0 + n * strength).clamp(0.0, 1.0);
                p[c] = (v * 255.0).round() as u8;
            }
        }
    }
}

fn hash21(x: f32, y: f32) -> f32 {
    let mut p3x = fract(x * 0.1031);
    let mut p3y = fract(y * 0.1031);
    let mut p3z = fract(x * 0.1031);
    let d = p3x * (p3y + 33.33) + p3y * (p3z + 33.33) + p3z * (p3x + 33.33);
    p3x = fract(p3x + d);
    p3y = fract(p3y + d);
    p3z = fract(p3z + d);
    fract((p3x + p3y) * p3z)
}

fn fract(v: f32) -> f32 {
    v - v.floor()
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
