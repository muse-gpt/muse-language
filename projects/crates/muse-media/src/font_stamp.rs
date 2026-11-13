//! Stamp glyph labels and lyric captions rasterized through the shared
//! `media.text` core.
//!
//! 归属裁决（G7-W3）：字形栅格与 run 绘制归 `media.text`（正式字形所有者）；
//! Muse 保留工程级字体资源解析（`MUSE_FONT` → 候选清单 → 显式错误）与词级
//! 时序、高亮选择、本地化 display 的语言语义。无可用字体不再静默跳绘——
//! 声明了文字 FX 却解析不到字体是显式错误（§11.4 禁静默回落）。

use anyhow::{Context, Result, bail};
use geometry_types::Rgba;
use media_image::Frame;
use media_text::{FontId, Fonts};
use muse_ir::{LyricAlign, LyricLayer, LyricLine, LyricWord};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

/// Process-shared `media.text` font set. Resolution happens once; a failed
/// resolution is a cached explicit error, never a silent skip.
fn shared_fonts() -> Result<std::sync::MutexGuard<'static, Fonts>> {
    static FONTS: OnceLock<Result<Mutex<Fonts>, String>> = OnceLock::new();
    let slot = FONTS.get_or_init(|| load_project_fonts().map(Mutex::new));
    match slot {
        Ok(mutex) => Ok(mutex.lock().expect("muse fonts mutex")),
        Err(message) => bail!("{message}"),
    }
}

/// Resolve font bytes at the project layer and hand them to the shared core.
fn load_project_fonts() -> Result<Fonts, String> {
    let bytes = resolve_font_bytes().map_err(|error| error.to_string())?;
    Fonts::from_bytes(&bytes, &bytes, &bytes)
        .map_err(|error| format!("muse font bytes rejected by media.text: {error}"))
}

/// `MUSE_FONT` is an explicit contract: set but unreadable fails. Otherwise
/// the first readable candidate wins; none found is an explicit error.
fn resolve_font_bytes() -> Result<Vec<u8>> {
    if let Ok(explicit) = std::env::var("MUSE_FONT") {
        let path = Path::new(explicit.trim());
        if !path.as_os_str().is_empty() {
            let bytes = std::fs::read(path)
                .with_context(|| format!("MUSE_FONT `{}` unreadable", path.display()))?;
            return Ok(bytes);
        }
    }
    for path in candidate_font_paths() {
        if let Ok(bytes) = std::fs::read(&path) {
            return Ok(bytes);
        }
    }
    bail!("no CJK-capable font found (set MUSE_FONT to a TTF/OTF/TTC path)");
}

/// Draw `label` centered with slam scale/opacity driven by shot progress.
pub fn draw_stamp_label(
    canvas: &mut Frame,
    label: &str,
    strength: f32,
    shot_progress: f32,
) -> Result<()> {
    let label = label.trim();
    if label.is_empty() {
        return Ok(());
    }
    let strength = strength.clamp(0.0, 1.0);
    if strength <= 0.0 {
        return Ok(());
    }
    let slam = smoothstep(0.0, 0.22, shot_progress);
    let scale_factor = 2.2 - 1.2 * slam;
    let opacity =
        strength * slam * (0.75 + 0.25 * (shot_progress * std::f32::consts::PI).sin().abs());
    if opacity <= 0.0 {
        return Ok(());
    }

    let height = canvas.height.max(1) as f32;
    let px = (height * 0.42 * scale_factor).clamp(12.0, height * 0.85);
    let mut fonts = shared_fonts()?;
    let width = fonts.measure(label, px, false, false);
    if width <= 0.0 {
        return Ok(());
    }
    let x = (canvas.width as f32 - width) * 0.5;
    // Optical centering: baseline below the vertical middle by ~0.36 em.
    let baseline = height * 0.5 + px * 0.36;
    let alpha = (opacity * 255.0).round() as u8;
    fonts.draw_line(canvas, label, x, baseline, px, FontId::Regular, Rgba([255, 120, 72, alpha]));
    Ok(())
}

/// Same layout as the caption path with an optional localized display
/// override and LyricCard opacity (`intensity` / focus dimming).
pub fn draw_lyric_caption(
    canvas: &mut Frame,
    layer: &LyricLayer,
    line: &LyricLine,
    active: Option<&LyricWord>,
    display_text: Option<&str>,
    opacity: f32,
) -> Result<()> {
    if line.words.is_empty() && display_text.is_none_or(|s| s.trim().is_empty()) {
        return Ok(());
    }
    let opacity = opacity.clamp(0.0, 1.0);
    if opacity <= 0.001 {
        return Ok(());
    }
    let short = (canvas.width.min(canvas.height)).max(1) as f32;
    let margin = (short * layer.safe_margin.clamp(0.0, 0.45) as f32).max(4.0);
    let px = if layer.size_px > 0.0 {
        layer.size_px as f32
    } else {
        (short * 0.055).clamp(14.0, short * 0.12)
    };
    let alpha = (opacity * 255.0).round() as u8;

    let mut fonts = shared_fonts()?;
    let gap = fonts.measure(" ", px, false, false) * 0.35;

    let mut runs: Vec<(String, [u8; 3], f32)> = Vec::new();
    let mut total_w = 0.0_f32;
    if let Some(text) = display_text.map(str::trim).filter(|s| !s.is_empty()) {
        let color = if active.is_some() { layer.highlight_color } else { layer.color };
        let width = fonts.measure(text, px, false, false);
        total_w = width;
        runs.push((text.to_owned(), color, width));
    } else {
        for (idx, word) in line.words.iter().enumerate() {
            let color = if active.is_some_and(|a| a.id == word.id) {
                layer.highlight_color
            } else {
                layer.color
            };
            let width = fonts.measure(&word.text, px, false, false);
            if idx > 0 {
                total_w += gap;
            }
            total_w += width;
            runs.push((word.text.clone(), color, width));
        }
    }
    if total_w <= 0.0 {
        return Ok(());
    }

    let max_w = (canvas.width as f32 - margin * 2.0).max(8.0);
    let origin_x = match layer.align {
        LyricAlign::Start => margin,
        LyricAlign::Center => ((canvas.width as f32 - total_w.min(max_w)) * 0.5).max(margin),
        LyricAlign::End => (canvas.width as f32 - margin - total_w.min(max_w)).max(margin),
    };
    let baseline = canvas.height as f32 - margin - px * 0.25;

    let mut x = origin_x;
    for (text, color, width) in runs {
        fonts.draw_line(
            canvas,
            &text,
            x,
            baseline,
            px,
            FontId::Regular,
            Rgba([color[0], color[1], color[2], alpha]),
        );
        x += width + gap;
        if x > canvas.width as f32 - margin {
            break;
        }
    }
    Ok(())
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn candidate_font_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    // Windows
    let windir = std::env::var_os("WINDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
    let fonts = windir.join("Fonts");
    // Prefer single-font TTF/OTF first — many TTC collections fail to parse.
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

#[cfg(test)]
mod tests {
    use super::*;
    use media_image::Frame;

    fn frame_with(fill: [u8; 4], width: u32, height: u32) -> Frame {
        let mut frame = Frame::new(width, height);
        for chunk in frame.pixels.chunks_exact_mut(4) {
            chunk.copy_from_slice(&fill);
        }
        frame
    }

    fn fonts_available() -> bool {
        resolve_font_bytes().is_ok()
    }

    #[test]
    fn draws_cjk_stamp_via_media_text() {
        if !fonts_available() {
            eprintln!("skip: no system CJK font");
            return;
        }
        let mut canvas = frame_with([11, 21, 36, 255], 128, 72);
        draw_stamp_label(&mut canvas, "残", 0.9, 0.5).expect("stamp draws");
        let warm = canvas
            .pixels
            .chunks_exact(4)
            .any(|p| p[0] > 180 && p[1] < 160 && p[2] < 120);
        assert!(warm, "expected vermilion stamp pixels from media.text");
    }

    #[test]
    fn draws_lyric_highlight_via_media_text() {
        if !fonts_available() {
            eprintln!("skip: no system CJK font");
            return;
        }
        let mut canvas = frame_with([11, 21, 36, 255], 320, 180);
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
        draw_lyric_caption(&mut canvas, &layer, &line, Some(&line.words[0]), None, 0.95)
            .expect("caption draws");
        let warm = canvas
            .pixels
            .chunks_exact(4)
            .any(|p| p[0] > 200 && p[1] > 180 && p[2] < 180);
        assert!(warm, "expected highlight lyric pixels");
    }

    #[test]
    fn missing_font_is_an_explicit_error_not_a_silent_skip() {
        // Only meaningful where no candidate font resolves; on dev machines
        // with CJK fonts this asserts the happy path returns Ok.
        if fonts_available() {
            let mut canvas = frame_with([0, 0, 0, 255], 32, 32);
            assert!(draw_stamp_label(&mut canvas, "字", 1.0, 0.5).is_ok());
        } else {
            let mut canvas = frame_with([0, 0, 0, 255], 32, 32);
            let error = draw_stamp_label(&mut canvas, "字", 1.0, 0.5)
                .expect_err("declared text without font must fail");
            assert!(error.to_string().contains("font"), "{error}");
        }
    }
}
