//! Rasterize stamp glyph labels with a system CJK-capable font.
//!
//! Falls back silently when no usable font is found (geometry stamp still draws).

use ab_glyph::{Font, FontArc, PxScale, ScaleFont, point};
use image::RgbImage;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Draw `label` centered with slam scale/opacity driven by shot progress.
pub fn draw_stamp_label(
    canvas: &mut RgbImage,
    label: &str,
    strength: f32,
    shot_progress: f32,
) {
    let label = label.trim();
    if label.is_empty() {
        return;
    }
    let strength = strength.clamp(0.0, 1.0);
    if strength <= 0.0 {
        return;
    }
    let Some(font) = system_font() else {
        return;
    };

    let slam = smoothstep(0.0, 0.22, shot_progress);
    let scale_factor = 2.2 - 1.2 * slam;
    let opacity = strength * slam * (0.75 + 0.25 * (shot_progress * std::f32::consts::PI).sin().abs());
    if opacity <= 0.0 {
        return;
    }

    let height = canvas.height().max(1) as f32;
    let px = (height * 0.42 * scale_factor).clamp(12.0, height * 0.85);
    let scale = PxScale::from(px);
    let scaled = font.as_scaled(scale);

    let mut glyphs = Vec::new();
    let mut caret = 0.0_f32;
    for ch in label.chars() {
        let id = font.glyph_id(ch);
        let glyph = id.with_scale_and_position(scale, point(caret, 0.0));
        caret += scaled.h_advance(id);
        glyphs.push(glyph);
    }
    if glyphs.is_empty() || caret <= 0.0 {
        return;
    }

    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;
    for glyph in &glyphs {
        if let Some(outlined) = font.outline_glyph(glyph.clone()) {
            let b = outlined.px_bounds();
            min_x = min_x.min(b.min.x);
            min_y = min_y.min(b.min.y);
            max_x = max_x.max(b.max.x);
            max_y = max_y.max(b.max.y);
        }
    }
    if !min_x.is_finite() {
        return;
    }

    let gw = (max_x - min_x).max(1.0);
    let gh = (max_y - min_y).max(1.0);
    let origin_x = (canvas.width() as f32 - gw) * 0.5 - min_x;
    let origin_y = (canvas.height() as f32 - gh) * 0.5 - min_y;

    for glyph in glyphs {
        let positioned = glyph.id.with_scale_and_position(
            scale,
            point(glyph.position.x + origin_x, glyph.position.y + origin_y),
        );
        let Some(outlined) = font.outline_glyph(positioned) else {
            continue;
        };
        let bounds = outlined.px_bounds();
        outlined.draw(|x, y, c| {
            if c <= 0.0 {
                return;
            }
            let px = bounds.min.x as i32 + x as i32;
            let py = bounds.min.y as i32 + y as i32;
            if px < 0 || py < 0 {
                return;
            }
            let (px, py) = (px as u32, py as u32);
            if px >= canvas.width() || py >= canvas.height() {
                return;
            }
            let cover = (c * opacity).clamp(0.0, 1.0);
            let p = canvas.get_pixel_mut(px, py);
            // Warm vermilion fill (P1 accent-ish), not pure white.
            p[0] = mix_u8(p[0], 255, cover);
            p[1] = mix_u8(p[1], 120, cover);
            p[2] = mix_u8(p[2], 72, cover);
        });
    }
}

fn mix_u8(src: u8, dst: u8, t: f32) -> u8 {
    (src as f32 * (1.0 - t) + dst as f32 * t).round() as u8
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn system_font() -> Option<&'static FontArc> {
    static FONT: OnceLock<Option<FontArc>> = OnceLock::new();
    FONT.get_or_init(|| {
        if let Ok(explicit) = std::env::var("MUSE_FONT") {
            if let Some(font) = load_font_file(Path::new(explicit.trim())) {
                return Some(font);
            }
        }
        for path in candidate_font_paths() {
            if let Some(font) = load_font_file(&path) {
                return Some(font);
            }
        }
        None
    })
    .as_ref()
}

fn load_font_file(path: &Path) -> Option<FontArc> {
    let data = std::fs::read(path).ok()?;
    FontArc::try_from_vec(data).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgb;

    #[test]
    fn draws_cjk_stamp_when_system_font_available() {
        let Some(_) = system_font() else {
            eprintln!("skip: no system CJK font");
            return;
        };
        let mut canvas = RgbImage::from_pixel(128, 72, Rgb([11, 21, 36]));
        draw_stamp_label(&mut canvas, "残", 0.9, 0.5);
        let warm = canvas.pixels().any(|p| p[0] > 180 && p[1] < 160 && p[2] < 120);
        assert!(warm, "expected vermilion stamp pixels from ab_glyph");
    }
}

fn candidate_font_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    // Windows
    let windir = std::env::var_os("WINDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
    let fonts = windir.join("Fonts");
    // Prefer single-font TTF/OTF first — ab_glyph rejects many TTC collections.
    for name in [
        "simhei.ttf",
        "SIMHEI.TTF",
        "msyh.ttf",
        "arialuni.ttf",
        "msyh.ttc",
        "msyhbd.ttc",
        "simsun.ttc",
        "msjh.ttc",
        "YuGothM.ttc",
    ] {
        paths.push(fonts.join(name));
    }
    // macOS
    for name in [
        "/System/Library/Fonts/PingFang.ttc",
        "/System/Library/Fonts/Hiragino Sans GB.ttc",
        "/Library/Fonts/Arial Unicode.ttf",
    ] {
        paths.push(PathBuf::from(name));
    }
    // Linux
    for name in [
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
    ] {
        paths.push(PathBuf::from(name));
    }
    paths
}
