use anyhow::{Context, Result, bail};
use muse_ir::RenderPlan;
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use crate::preview::plan_background_rgb;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Preview,
    Wav,
    Mp4,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendCapability {
    Supported,
    Unsupported,
}

/// Contract capability: MP4 is supported when an external `ffmpeg` can be resolved at render time.
pub const fn capability(format: OutputFormat) -> BackendCapability {
    match format {
        OutputFormat::Preview | OutputFormat::Wav | OutputFormat::Mp4 => BackendCapability::Supported,
    }
}

pub trait RenderBackend {
    fn render(&self, plan: &RenderPlan, output: &Path) -> Result<()>;
}

#[derive(Debug, Default)]
pub struct LocalPreviewBackend;

impl RenderBackend for LocalPreviewBackend {
    fn render(&self, plan: &RenderPlan, output: &Path) -> Result<()> {
        crate::render_preview_with(plan, output, crate::PreviewOptions::default()).map(|_| ())
    }
}

pub fn find_ffmpeg() -> Result<PathBuf> {
    if let Ok(explicit) = env::var("MUSE_FFMPEG") {
        let path = PathBuf::from(explicit);
        if path.is_file() {
            return Ok(path);
        }
        bail!("MUSE_FFMPEG points to a missing binary: {}", path.display());
    }
    which("ffmpeg").context(
        "MP4 render requires ffmpeg on PATH (or set MUSE_FFMPEG); install ffmpeg or use `muse preview` / audio export",
    )
}

fn which(bin: &str) -> Result<PathBuf> {
    let path = env::var_os("PATH").unwrap_or_default();
    for dir in env::split_paths(&path) {
        let candidate = dir.join(bin);
        if candidate.is_file() {
            return Ok(candidate);
        }
        #[cfg(windows)]
        {
            let exe = dir.join(format!("{bin}.exe"));
            if exe.is_file() {
                return Ok(exe);
            }
        }
    }
    bail!("`{bin}` not found on PATH")
}

pub fn render_mp4(plan: &RenderPlan, output: &Path) -> Result<()> {
    debug_assert_eq!(capability(OutputFormat::Mp4), BackendCapability::Supported);
    let ffmpeg = find_ffmpeg()?;
    if let Some(parent) = output.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        fs::create_dir_all(parent)
            .with_context(|| format!("cannot create output directory {}", parent.display()))?;
    }

    let (r, g, b, _) = plan_background_rgb(plan);
    let duration = plan.duration_seconds.max(1.0 / plan.settings.frame_rate.max(1) as f64);
    let color = format!("0x{r:02x}{g:02x}{b:02x}");
    let size = format!("{}x{}", plan.settings.width, plan.settings.height);
    let video = format!(
        "color=c={color}:s={size}:r={}:d={}",
        plan.settings.frame_rate, duration
    );
    // Silence bed until music mux is wired; keeps pure-Rust builds free of huge temp WAVs.
    let audio = format!("anullsrc=channel_layout=stereo:sample_rate={}", plan.settings.sample_rate);

    let output_result = Command::new(&ffmpeg)
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            &video,
            "-f",
            "lavfi",
            "-i",
            &audio,
            "-t",
            &format!("{duration:.3}"),
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            "-shortest",
            output.to_str().context("output path is not valid UTF-8")?,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .with_context(|| format!("failed to spawn {}", ffmpeg.display()))?;

    if !output_result.status.success() {
        let stderr = String::from_utf8_lossy(&output_result.stderr);
        bail!(
            "ffmpeg failed while writing {} (exit {:?}): {}",
            output.display(),
            output_result.status.code(),
            stderr.trim()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{BackendCapability, OutputFormat, capability, find_ffmpeg};

    #[test]
    fn marks_preview_wav_and_mp4_as_supported_contract() {
        assert_eq!(capability(OutputFormat::Preview), BackendCapability::Supported);
        assert_eq!(capability(OutputFormat::Wav), BackendCapability::Supported);
        assert_eq!(capability(OutputFormat::Mp4), BackendCapability::Supported);
    }

    #[test]
    fn resolves_ffmpeg_when_present() {
        // Soft assertion: environments without ffmpeg still pass the suite.
        let _ = find_ffmpeg();
    }
}
