//! Deterministic frame composition from a [`RenderPlan`].
use anyhow::{Context, Result};
use image::{Rgb, RgbImage, imageops};
use muse_ir::RenderPlan;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

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
pub fn compose_frame(
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
    // Progress cue so solid / static plates still show motion in short previews.
    let progress_x =
        ((t.clamp(0.0, 1.0) * (width.saturating_sub(1) as f32)) as u32).min(width.saturating_sub(1));
    for y in (height.saturating_sub(4))..height {
        canvas.put_pixel(progress_x, y, Rgb([220, 220, 220]));
    }
    Ok(canvas)
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
