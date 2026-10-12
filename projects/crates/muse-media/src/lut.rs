//! Gradient-map LUTs for post grading (256×1, luma → palette colour).

use anyhow::{Context, Result, bail};
use image::{Rgb, RgbImage, Rgba, RgbaImage};
use std::path::Path;

/// Resolve a LUT id (`P2`) or project-relative PNG path into a 256×1 RGBA strip.
pub fn resolve_lut(name: &str, asset_root: &Path) -> Result<RgbaImage> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        bail!("lut name must be non-empty");
    }
    if looks_like_path(trimmed) {
        let path = if Path::new(trimmed).is_absolute() {
            Path::new(trimmed).to_path_buf()
        } else {
            asset_root.join(trimmed)
        };
        let img = image::open(&path)
            .with_context(|| format!("cannot open lut `{}` ({})", trimmed, path.display()))?
            .to_rgba8();
        return Ok(resample_to_strip(&img));
    }
    builtin_gradmap(trimmed)
}

fn looks_like_path(name: &str) -> bool {
    name.contains('/')
        || name.contains('\\')
        || name.contains('.')
        || name.starts_with("assets")
}

fn builtin_gradmap(id: &str) -> Result<RgbaImage> {
    let key = id.trim().to_ascii_uppercase().replace('-', "_");
    let stops: &[(f32, [u8; 3])] = match key.as_str() {
        "P1" | "P1_OUMAGATOKI" => &[
            (0.0, [12, 8, 18]),
            (0.2, [48, 18, 36]),
            (0.45, [200, 55, 58]),
            (0.7, [255, 165, 82]),
            (1.0, [255, 228, 196]),
        ],
        "P2" | "P2_ZANSHO" => &[
            (0.0, [11, 21, 36]),
            (0.25, [30, 53, 82]),
            (0.45, [63, 98, 132]),
            (0.6, [216, 84, 42]),
            (0.8, [244, 166, 64]),
            (1.0, [233, 217, 184]),
        ],
        "P3" | "P3_FUMIKIRI" => &[
            (0.0, [8, 8, 10]),
            (0.35, [40, 12, 16]),
            (0.55, [200, 40, 48]),
            (1.0, [245, 245, 245]),
        ],
        "P4" | "P4_HOTARUBI" => &[
            (0.0, [6, 12, 28]),
            (0.35, [24, 48, 72]),
            (0.65, [80, 160, 140]),
            (1.0, [220, 240, 200]),
        ],
        "P5" | "P5_TAISHOKU" => &[
            (0.0, [20, 28, 40]),
            (0.4, [90, 110, 130]),
            (0.7, [180, 190, 200]),
            (1.0, [240, 244, 248]),
        ],
        "IDENTITY" | "NONE" => return Ok(identity_strip()),
        other => bail!("unknown built-in lut `{other}` (use P1–P5 or a .png path)"),
    };
    Ok(stops_to_strip(stops))
}

fn identity_strip() -> RgbaImage {
    let mut img = RgbaImage::new(256, 1);
    for x in 0..256u32 {
        let v = x as u8;
        img.put_pixel(x, 0, Rgba([v, v, v, 255]));
    }
    img
}

fn stops_to_strip(stops: &[(f32, [u8; 3])]) -> RgbaImage {
    let mut img = RgbaImage::new(256, 1);
    for x in 0..256u32 {
        let t = x as f32 / 255.0;
        let rgb = sample_stops(stops, t);
        img.put_pixel(x, 0, Rgba([rgb[0], rgb[1], rgb[2], 255]));
    }
    img
}

fn sample_stops(stops: &[(f32, [u8; 3])], t: f32) -> [u8; 3] {
    let t = t.clamp(0.0, 1.0);
    if stops.is_empty() {
        return [0, 0, 0];
    }
    if t <= stops[0].0 {
        return stops[0].1;
    }
    for window in stops.windows(2) {
        let (t0, c0) = window[0];
        let (t1, c1) = window[1];
        if t <= t1 {
            let u = if (t1 - t0).abs() < f32::EPSILON {
                0.0
            } else {
                (t - t0) / (t1 - t0)
            };
            return [
                lerp_u8(c0[0], c1[0], u),
                lerp_u8(c0[1], c1[1], u),
                lerp_u8(c0[2], c1[2], u),
            ];
        }
    }
    stops[stops.len() - 1].1
}

fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * t).round() as u8
}

fn resample_to_strip(img: &RgbaImage) -> RgbaImage {
    if img.width() == 256 && img.height() == 1 {
        return img.clone();
    }
    let mut out = RgbaImage::new(256, 1);
    let src_w = img.width().max(1);
    let y = img.height() / 2;
    for x in 0..256u32 {
        let sx = ((x as f32 / 255.0) * (src_w - 1) as f32).round() as u32;
        out.put_pixel(x, 0, *img.get_pixel(sx.min(src_w - 1), y.min(img.height() - 1)));
    }
    out
}

/// Apply gradient-map grading to an RGB canvas (`lut_mix` in `[0, 1]`).
pub fn apply_lut_rgb(canvas: &mut RgbImage, lut: &RgbaImage, mix: f32) {
    let mix = mix.clamp(0.0, 1.0);
    if mix <= 0.0 {
        return;
    }
    let strip = resample_to_strip(lut);
    for pixel in canvas.pixels_mut() {
        let r = pixel[0] as f32 / 255.0;
        let g = pixel[1] as f32 / 255.0;
        let b = pixel[2] as f32 / 255.0;
        let luma = (0.2126 * r + 0.7152 * g + 0.0722 * b).clamp(0.0, 1.0);
        let idx = (luma * 255.0).round() as u32;
        let graded = strip.get_pixel(idx.min(255), 0);
        let out = Rgb([
            mix_channel(pixel[0], graded[0], mix),
            mix_channel(pixel[1], graded[1], mix),
            mix_channel(pixel[2], graded[2], mix),
        ]);
        *pixel = out;
    }
}

fn mix_channel(src: u8, graded: u8, mix: f32) -> u8 {
    (src as f32 * (1.0 - mix) + graded as f32 * mix).round() as u8
}
