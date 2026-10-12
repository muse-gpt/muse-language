use anyhow::Result;
use image::{Rgb, RgbImage};
use muse_ir::RenderPlan;
use serde::Serialize;
use std::{fs, path::Path};

#[derive(Debug, Serialize)]
pub struct PreviewManifest {
    pub frames: usize,
    pub width: u32,
    pub height: u32,
    pub duration_seconds: f64,
    pub audio_seconds: f64,
    pub audio_file: String,
    pub background_color: Option<String>,
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

pub fn render_preview(plan: &RenderPlan, out: &Path, scale: f32) -> Result<PreviewManifest> {
    let width = ((plan.settings.width as f32 * scale).max(16.0)) as u32;
    let height = ((plan.settings.height as f32 * scale).max(16.0)) as u32;
    fs::create_dir_all(out)?;
    let (r, g, b, background_color) = plan_background_rgb(plan);
    let frames = ((plan.duration_seconds * plan.settings.frame_rate as f64).ceil() as usize).clamp(1, 240);
    let fill = Rgb([r, g, b]);
    for index in 0..frames {
        let mut image = RgbImage::from_pixel(width, height, fill);
        // Keep a thin progress bar so consecutive solid frames are distinguishable.
        let progress_x = ((index as f32 / frames.saturating_sub(1).max(1) as f32) * (width.saturating_sub(1) as f32))
            as u32;
        for y in (height.saturating_sub(4))..height {
            image.put_pixel(progress_x.min(width.saturating_sub(1)), y, Rgb([220, 220, 220]));
        }
        image.save(out.join(format!("frame-{index:04}.png")))?;
    }
    let audio_seconds = plan.duration_seconds.min(10.0);
    crate::write_wav(&out.join("preview.wav"), audio_seconds, plan.settings.sample_rate)?;
    let manifest = PreviewManifest {
        frames,
        width,
        height,
        duration_seconds: plan.duration_seconds,
        audio_seconds,
        audio_file: "preview.wav".into(),
        background_color,
    };
    fs::write(out.join("manifest.json"), serde_json::to_vec_pretty(&manifest)?)?;
    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        env, fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn fills_frames_with_declared_background() {
        let plan = muse_ir::compile(
            r##"timeline { act main : 0..1 } visual for main { background color "#0b1524" }"##,
            Default::default(),
        )
        .plan
        .expect("valid plan");
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = env::temp_dir().join(format!("muse-media-preview-{stamp}"));
        let out = root.join("preview");
        let manifest = render_preview(&plan, &out, 0.1).unwrap();
        assert_eq!(manifest.background_color.as_deref(), Some("#0b1524"));
        let png = image::open(out.join("frame-0000.png")).unwrap().to_rgb8();
        assert_eq!(*png.get_pixel(0, 0), Rgb([0x0b, 0x15, 0x24]));
        let _ = fs::remove_dir_all(root);
    }
}
