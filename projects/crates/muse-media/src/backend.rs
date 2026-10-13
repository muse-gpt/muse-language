use anyhow::{Context, Result, bail};
use muse_ir::RenderPlan;
use std::{
    collections::HashMap,
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::compose::{ComposeOptions, compose_frame};

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

#[derive(Debug, Clone, Default)]
pub struct Mp4Options {
    pub asset_root: PathBuf,
    /// Optional pre-rendered WAV to mux (composed score / guide). Falls back to silence.
    pub audio_wav: Option<PathBuf>,
    /// Optional hard cap on encoded seconds (useful for smoke tests).
    pub max_duration_seconds: Option<f64>,
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
    render_mp4_with(plan, output, Mp4Options::default())
}

pub fn render_mp4_with(plan: &RenderPlan, output: &Path, options: Mp4Options) -> Result<()> {
    debug_assert_eq!(capability(OutputFormat::Mp4), BackendCapability::Supported);
    let ffmpeg = find_ffmpeg()?;
    if let Some(parent) = output.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        fs::create_dir_all(parent)
            .with_context(|| format!("cannot create output directory {}", parent.display()))?;
    }

    let duration = plan.duration_seconds.max(1.0 / plan.settings.frame_rate.max(1) as f64);
    let duration = options
        .max_duration_seconds
        .map(|cap| duration.min(cap.max(0.1)))
        .unwrap_or(duration);

    let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let tmp = env::temp_dir().join(format!("muse-mp4-{stamp}"));
    fs::create_dir_all(&tmp)?;

    let audio_input = if let Some(wav) = options.audio_wav.as_ref().filter(|path| path.is_file()) {
        wav.clone()
    } else {
        let silence = tmp.join("silence.wav");
        crate::write_wav(&silence, duration, plan.settings.sample_rate)?;
        silence
    };

    // Always compose through the plan (FX / shots / grade), even when there are no images.
    // Truncated `--max-duration` keeps wall-clock time via `normalized_time_at_frame`.
    let frames_dir = tmp.join("frames");
    fs::create_dir_all(&frames_dir)?;
    let frames = ((duration * plan.settings.frame_rate as f64).ceil() as usize).max(1);
    let compose = ComposeOptions {
        asset_root: if options.asset_root.as_os_str().is_empty() {
            PathBuf::from(".")
        } else {
            options.asset_root.clone()
        },
    };
    let mut cache = HashMap::new();
    for index in 0..frames {
        let t = plan.normalized_time_at_frame(index);
        let image = compose_frame(
            plan,
            plan.settings.width,
            plan.settings.height,
            t,
            &compose,
            &mut cache,
        )?;
        image.save(frames_dir.join(format!("frame-{index:04}.png")))?;
    }
    let pattern = frames_dir.join("frame-%04d.png");
    let fr = plan.settings.frame_rate.to_string();
    let dur = format!("{duration:.3}");
    run_ffmpeg(
        &ffmpeg,
        &[
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-framerate",
            &fr,
            "-i",
            pattern.to_str().context("frame pattern is not valid UTF-8")?,
            "-i",
            audio_input.to_str().context("audio path is not valid UTF-8")?,
            "-t",
            &dur,
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            "-shortest",
            output.to_str().context("output path is not valid UTF-8")?,
        ],
    )?;

    let _ = fs::remove_dir_all(&tmp);
    Ok(())
}

fn run_ffmpeg(ffmpeg: &Path, args: &[&str]) -> Result<()> {
    let output_result = Command::new(ffmpeg)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .with_context(|| format!("failed to spawn {}", ffmpeg.display()))?;
    if !output_result.status.success() {
        let stderr = String::from_utf8_lossy(&output_result.stderr);
        bail!(
            "ffmpeg failed while writing (exit {:?}): {}",
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
        let _ = find_ffmpeg();
    }
}
