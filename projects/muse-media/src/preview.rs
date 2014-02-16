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
    pub audio_file: String,
}
pub fn render_preview(plan: &RenderPlan, out: &Path, scale: f32) -> Result<PreviewManifest> {
    let width = ((plan.settings.width as f32 * scale).max(16.0)) as u32;
    let height = ((plan.settings.height as f32 * scale).max(16.0)) as u32;
    fs::create_dir_all(out)?;
    let frames = ((plan.duration_seconds * plan.settings.frame_rate as f64).ceil() as usize).clamp(1, 240);
    for index in 0..frames {
        let t = index as f32 / (frames.saturating_sub(1).max(1)) as f32;
        let mut image = RgbImage::new(width, height);
        for (x, y, pixel) in image.enumerate_pixels_mut() {
            let pulse = ((t * 12.0 + x as f32 / width as f32 * 4.0).sin() * 20.0 + 30.0) as u8;
            *pixel = Rgb([
                ((x * 255 / width) as u8).saturating_add(pulse),
                ((y * 120 / height) as u8).saturating_add(pulse),
                80u8.saturating_add(pulse),
            ]);
        }
        image.save(out.join(format!("frame-{index:04}.png")))?;
    }
    crate::write_wav(
        &out.join("preview.wav"),
        plan.duration_seconds.min(10.0),
        plan.settings.sample_rate,
    )?;
    let manifest = PreviewManifest {
        frames,
        width,
        height,
        duration_seconds: plan.duration_seconds,
        audio_file: "preview.wav".into(),
    };
    fs::write(out.join("manifest.json"), serde_json::to_vec_pretty(&manifest)?)?;
    Ok(manifest)
}
