//! Deterministic frame composition from a [`RenderPlan`].
use anyhow::{Context, Result, bail};
use image::{Rgb, RgbImage, imageops};
use muse_ir::RenderPlan;
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
pub fn compose_frame(
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

fn compose_frame_cpu(
    plan: &RenderPlan,
    width: u32,
    height: u32,
    t: f32,
    options: &ComposeOptions,
    cache: &mut HashMap<String, RgbImage>,
) -> Result<RgbImage> {
    let (r, g, b, _) = plan_background_rgb(plan);
    let mut canvas = RgbImage::from_pixel(width, height, Rgb([r, g, b]));
    let images: Vec<&str> = plan
        .visuals
        .iter()
        .flat_map(|layer| layer.images.iter().map(String::as_str))
        .collect();
    for uri in images {
        let plate = load_image(uri, options, cache)?;
        blit_cover(&mut canvas, &plate);
    }
    if let Some(strength) = plan_vignette(plan) {
        apply_vignette_rgb(&mut canvas, strength);
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

    let rgba = compositor.compose_layers(
        &plan.visuals,
        &GpuComposeOptions {
            width,
            height,
            asset_root: options.asset_root.clone(),
        },
    )?;
    let mut canvas = RgbImage::from_fn(width, height, |x, y| {
        let p = rgba.get_pixel(x, y);
        Rgb([p[0], p[1], p[2]])
    });
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
    plan.visuals.iter().rev().find_map(|layer| layer.vignette)
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
