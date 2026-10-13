//! Rasterize stamp glyph labels and lyric captions with a system CJK-capable font.
//!
//! Falls back silently when no usable font is found (geometry stamp still draws).

use ab_glyph::{Font, FontArc, PxScale, ScaleFont, point};
use image::RgbImage;
use muse_ir::{LyricAlign, LyricLayer, LyricLine, LyricWord};
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

/// Draw the active lyric line near the bottom safe area; highlight the active word.
pub fn draw_lyric_caption(
    canvas: &mut RgbImage,
    layer: &LyricLayer,
    line: &LyricLine,
    active: Option<&LyricWord>,
) {
    if line.words.is_empty() {
        return;
    }
    let Some(font) = system_font() else {
        return;
    };
    let short = canvas.width().min(canvas.height()).max(1) as f32;
    let margin = (short * layer.safe_margin.clamp(0.0, 0.45) as f32).max(4.0);
    let px = if layer.size_px > 0.0 {
        layer.size_px as f32
    } else {
        (short * 0.055).clamp(14.0, short * 0.12)
    };
    let scale = PxScale::from(px);
    let scaled = font.as_scaled(scale);
    let gap = scaled.h_advance(font.glyph_id(' ')) * 0.35;

    let mut runs: Vec<(String, [u8; 3], f32)> = Vec::new();
    let mut total_w = 0.0_f32;
    for (idx, word) in line.words.iter().enumerate() {
        let color = if active.is_some_and(|a| a.id == word.id) {
            layer.highlight_color
        } else {
            layer.color
        };
        let mut caret = 0.0_f32;
        for ch in word.text.chars() {
            caret += scaled.h_advance(font.glyph_id(ch));
        }
        if idx > 0 {
            total_w += gap;
        }
        total_w += caret;
        runs.push((word.text.clone(), color, caret));
    }
    if total_w <= 0.0 {
        return;
    }

    let max_w = (canvas.width() as f32 - margin * 2.0).max(8.0);
    let origin_x = match layer.align {
        LyricAlign::Start => margin,
        LyricAlign::Center => ((canvas.width() as f32 - total_w.min(max_w)) * 0.5).max(margin),
        LyricAlign::End => (canvas.width() as f32 - margin - total_w.min(max_w)).max(margin),
    };
    let baseline = canvas.height() as f32 - margin - px * 0.25;

    let mut x = origin_x;
    for (text, color, width) in runs {
        draw_text_run(canvas, font, &text, scale, x, baseline, color, 0.95);
        x += width + gap;
        if x > canvas.width() as f32 - margin {
            break;
        }
    }
}

fn draw_text_run(
    canvas: &mut RgbImage,
    font: &FontArc,
    text: &str,
    scale: PxScale,
    origin_x: f32,
    baseline: f32,
    color: [u8; 3],
    opacity: f32,
) {
    let scaled = font.as_scaled(scale);
    let mut caret = 0.0_f32;
    for ch in text.chars() {
        let id = font.glyph_id(ch);
        let glyph = id.with_scale_and_position(scale, point(origin_x + caret, baseline));
        caret += scaled.h_advance(id);
        let Some(outlined) = font.outline_glyph(glyph) else {
            continue;
        };
        let bounds = outlined.px_bounds();
        outlined.draw(|gx, gy, c| {
            if c <= 0.0 {
                return;
            }
            let px = bounds.min.x as i32 + gx as i32;
            let py = bounds.min.y as i32 + gy as i32;
            if px < 0 || py < 0 {
                return;
            }
            let (px, py) = (px as u32, py as u32);
            if px >= canvas.width() || py >= canvas.height() {
                return;
            }
            let cover = (c * opacity).clamp(0.0, 1.0);
            let p = canvas.get_pixel_mut(px, py);
            p[0] = mix_u8(p[0], color[0], cover);
            p[1] = mix_u8(p[1], color[1], cover);
            p[2] = mix_u8(p[2], color[2], cover);
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

    #[test]
    fn draws_lyric_highlight_when_system_font_available() {
        let Some(_) = system_font() else {
            eprintln!("skip: no system CJK font");
            return;
        };
        let mut canvas = RgbImage::from_pixel(320, 180, Rgb([11, 21, 36]));
        let line = LyricLine {
            id: "L1".into(),
            cue_id: Some("C1".into()),
            words: vec![
                LyricWord {
                    id: "W1".into(),
                    text: "残".into(),
                    start: 0.0,
                    end: 0.5,
                },
                LyricWord {
                    id: "W2".into(),
                    text: "响".into(),
                    start: 0.5,
                    end: 1.0,
                },
            ],
        };
        let layer = LyricLayer {
            id: "main_layer".into(),
            track: "main".into(),
            role: muse_ir::LyricLayerRole::Foreground,
            size_px: 36.0,
            color: [200, 200, 210],
            highlight_color: [255, 224, 138],
            align: LyricAlign::Center,
            safe_margin: 0.08,
            post_group: Some("none".into()),
        };
        draw_lyric_caption(&mut canvas, &layer, &line, Some(&line.words[0]));
        let warm = canvas
            .pixels()
            .any(|p| p[0] > 200 && p[1] > 180 && p[2] < 180);
        assert!(warm, "expected highlight lyric pixels");
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
