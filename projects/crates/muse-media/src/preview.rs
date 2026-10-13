use anyhow::Result;
use muse_ir::RenderPlan;
use serde::Serialize;
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use crate::compose::{ComposeOptions, compose_frame, plan_background_rgb};

#[derive(Debug, Clone)]
pub struct PreviewOptions {
    pub scale: f32,
    pub max_frames: usize,
    pub max_audio_seconds: f64,
    pub asset_root: PathBuf,
}

impl Default for PreviewOptions {
    fn default() -> Self {
        Self {
            scale: 0.1,
            max_frames: 240,
            max_audio_seconds: 10.0,
            asset_root: PathBuf::from("."),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct PreviewManifest {
    pub frames: usize,
    pub width: u32,
    pub height: u32,
    pub scale: f32,
    /// Full plan duration from the compile contract.
    pub plan_duration_seconds: f64,
    /// Duration actually covered by written frames (`frames / frame_rate`).
    pub rendered_duration_seconds: f64,
    /// Duration of the guide WAV (may be shorter than rendered video).
    pub audio_seconds: f64,
    pub audio_file: String,
    pub background_color: Option<String>,
    pub images: Vec<String>,
    /// Shot ids declared on the plan (may be empty).
    pub shots: Vec<String>,
    /// Distinct shot ids encountered while rendering frames (normalized time).
    pub active_shots: Vec<String>,
}

pub fn render_preview(plan: &RenderPlan, out: &Path, scale: f32) -> Result<PreviewManifest> {
    render_preview_with(
        plan,
        out,
        PreviewOptions {
            scale,
            ..PreviewOptions::default()
        },
    )
}

pub fn render_preview_with(plan: &RenderPlan, out: &Path, options: PreviewOptions) -> Result<PreviewManifest> {
    let scale = options.scale.clamp(0.01, 1.0);
    let width = ((plan.settings.width as f32 * scale).max(16.0)) as u32;
    let height = ((plan.settings.height as f32 * scale).max(16.0)) as u32;
    fs::create_dir_all(out)?;
    let (_, _, _, background_color) = plan_background_rgb(plan);
    let full_frames =
        ((plan.duration_seconds * plan.settings.frame_rate as f64).ceil() as usize).max(1);
    let frames = full_frames.min(options.max_frames.max(1));
    let compose = ComposeOptions {
        asset_root: options.asset_root.clone(),
    };
    let mut cache = HashMap::new();
    let mut active_shots = Vec::new();
    for index in 0..frames {
        // Wall-clock sampling: truncated previews cover plan start, not a full-timeline squeeze.
        let t = plan.normalized_time_at_frame(index);
        if let Some(shot) = plan.shot_at_normalized(t) {
            if !active_shots.iter().any(|id| id == &shot.id) {
                active_shots.push(shot.id.clone());
            }
        }
        let image = compose_frame(plan, width, height, t, &compose, &mut cache)?;
        image.save(out.join(format!("frame-{index:04}.png")))?;
    }
    let rendered_duration_seconds = frames as f64 / plan.settings.frame_rate.max(1) as f64;
    let audio_seconds = plan
        .duration_seconds
        .min(options.max_audio_seconds.max(0.0))
        .min(rendered_duration_seconds);
    crate::write_wav(&out.join("preview.wav"), audio_seconds, plan.settings.sample_rate)?;
    let images = plan
        .visuals
        .iter()
        .flat_map(|layer| layer.images.clone())
        .collect();
    let shots = plan.shots.iter().map(|shot| shot.id.clone()).collect();
    let manifest = PreviewManifest {
        frames,
        width,
        height,
        scale,
        plan_duration_seconds: plan.duration_seconds,
        rendered_duration_seconds,
        audio_seconds,
        audio_file: "preview.wav".into(),
        background_color,
        images,
        shots,
        active_shots,
    };
    fs::write(out.join("manifest.json"), serde_json::to_vec_pretty(&manifest)?)?;
    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, RgbImage};
    use std::{
        env, fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn fills_frames_with_declared_background() {
        // Exact RGB asserts use the CPU path (GPU sRGB clear can round differently).
        unsafe {
            env::set_var("MUSE_COMPOSITOR", "cpu");
        }
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
        unsafe {
            env::remove_var("MUSE_COMPOSITOR");
        }
    }

    #[test]
    fn truncated_preview_keeps_wall_clock_shots() {
        unsafe {
            env::set_var("MUSE_COMPOSITOR", "cpu");
        }
        // Long plan: late shot must not appear when preview is capped to ~1s of frames.
        let plan = muse_ir::compile(
            "timeline { act main : 0..100 shot early : 0..4 scene a shot late : 80..90 scene b } tempo { at 0 bpm 120 }",
            Default::default(),
        )
        .plan
        .expect("valid plan");
        assert!(plan.duration_seconds > 50.0);
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = env::temp_dir().join(format!("muse-media-clock-{stamp}"));
        let out = root.join("preview");
        let manifest = render_preview_with(
            &plan,
            &out,
            PreviewOptions {
                scale: 0.05,
                max_frames: plan.settings.frame_rate.max(1) as usize, // ~1s
                max_audio_seconds: 1.0,
                asset_root: root.clone(),
            },
        )
        .unwrap();
        assert_eq!(manifest.active_shots, ["early"]);
        assert!((manifest.rendered_duration_seconds - 1.0).abs() < 0.05);
        let _ = fs::remove_dir_all(root);
        unsafe {
            env::remove_var("MUSE_COMPOSITOR");
        }
    }

    #[test]
    fn composites_image_layer_over_background() {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = env::temp_dir().join(format!("muse-media-image-{stamp}"));
        let textures = root.join("assets/textures");
        fs::create_dir_all(&textures).unwrap();
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
        let out = root.join("preview");
        let manifest = render_preview_with(
            &plan,
            &out,
            PreviewOptions {
                scale: 0.05,
                max_frames: 2,
                max_audio_seconds: 1.0,
                asset_root: root.clone(),
            },
        )
        .unwrap();
        assert_eq!(manifest.images, ["assets/textures/plate.png"]);
        let png = image::open(out.join("frame-0000.png")).unwrap().to_rgb8();
        assert_eq!(*png.get_pixel(0, 0), Rgb([255, 0, 0]));
        let _ = fs::remove_dir_all(root);
    }
}
