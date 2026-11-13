use anyhow::{Context, Result, bail};
use media_codec::{Encoder, MediaLibrary};
use muse_ir::RenderPlan;
use serde::Serialize;
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::compose::{ComposeOptions, compose_frame};
use crate::wav::placeholder_pcm_stereo;

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

/// Which MP4 encode path a render uses. Chosen explicitly — there is no
/// silent fallback from one path to the other mid-render.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mp4EncoderPath {
    /// Media Foundation H.264/AAC encoder (Windows, the default there).
    MediaFoundation,
    /// External `ffmpeg` host binary resolved from `MUSE_FFMPEG` or PATH.
    Ffmpeg,
}

impl Mp4EncoderPath {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MediaFoundation => "mf",
            Self::Ffmpeg => "ffmpeg",
        }
    }
}

/// Resolve the encode path: `MUSE_MP4_ENCODER=mf|ffmpeg`, defaulting to `mf`
/// on Windows and `ffmpeg` elsewhere. Unknown values fail explicitly.
pub fn select_mp4_encoder() -> Result<Mp4EncoderPath> {
    match env::var("MUSE_MP4_ENCODER") {
        Ok(raw) => {
            let raw = raw.trim().to_ascii_lowercase();
            if raw.is_empty() || raw == "mf" {
                return Ok(Mp4EncoderPath::MediaFoundation);
            }
            if raw == "ffmpeg" {
                return Ok(Mp4EncoderPath::Ffmpeg);
            }
            bail!("unknown MUSE_MP4_ENCODER `{raw}` (expected `mf` or `ffmpeg`)")
        }
        Err(_) => Ok(if cfg!(windows) {
            Mp4EncoderPath::MediaFoundation
        } else {
            Mp4EncoderPath::Ffmpeg
        }),
    }
}

/// Availability of both encode paths for `muse doctor`. Probing the Media
/// Foundation encoder writes and removes one throwaway one-frame file.
#[derive(Debug, Clone, Serialize)]
pub struct Mp4EncoderStatus {
    pub selected: &'static str,
    pub media_foundation_available: bool,
    pub ffmpeg_available: bool,
    pub message: String,
}

pub fn mp4_encoder_status() -> Mp4EncoderStatus {
    let (selected, message) = match select_mp4_encoder() {
        Ok(path) => (path.as_str(), String::new()),
        Err(error) => (
            if cfg!(windows) { "mf" } else { "ffmpeg" },
            error.to_string(),
        ),
    };
    let (available, probe_message) = probe_media_foundation();
    Mp4EncoderStatus {
        selected,
        media_foundation_available: available,
        ffmpeg_available: find_ffmpeg().is_ok(),
        message: if message.is_empty() {
            probe_message
        } else {
            message
        },
    }
}

fn probe_media_foundation() -> (bool, String) {
    if !cfg!(windows) {
        return (false, "Media Foundation is Windows-only".into());
    }
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let tmp = env::temp_dir().join(format!("muse-mf-probe-{stamp}.mp4"));
    let attempt = (|| -> Result<(), media_types::Error> {
        let mut encoder = Encoder::open(&tmp, 16, 16, 1, 2_000_000, 44_100, false)?;
        // One black 16x16 NV12 frame (Y plane then interleaved UV).
        let nv12 = vec![0u8; 16 * 16 + 16 * 8];
        encoder.write_video(&nv12, 0, 1)?;
        encoder.finish()
    })();
    let _ = fs::remove_file(&tmp);
    match attempt {
        Ok(()) => (true, String::new()),
        Err(error) => (false, format!("Media Foundation encoder unavailable: {error}")),
    }
}

pub fn render_mp4(plan: &RenderPlan, output: &Path) -> Result<()> {
    render_mp4_with(plan, output, Mp4Options::default())
}

pub fn render_mp4_with(plan: &RenderPlan, output: &Path, options: Mp4Options) -> Result<()> {
    match select_mp4_encoder()? {
        Mp4EncoderPath::MediaFoundation => render_mp4_media_foundation(plan, output, &options),
        Mp4EncoderPath::Ffmpeg => render_mp4_ffmpeg(plan, output, &options),
    }
}

fn plan_duration(plan: &RenderPlan, options: &Mp4Options) -> f64 {
    let duration = plan.duration_seconds.max(1.0 / plan.settings.frame_rate.max(1) as f64);
    options
        .max_duration_seconds
        .map(|cap| duration.min(cap.max(0.1)))
        .unwrap_or(duration)
}

fn compose_options(options: &Mp4Options) -> ComposeOptions {
    ComposeOptions {
        asset_root: if options.asset_root.as_os_str().is_empty() {
            PathBuf::from(".")
        } else {
            options.asset_root.clone()
        },
    }
}

fn ensure_output_parent(output: &Path) -> Result<()> {
    if let Some(parent) = output.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        fs::create_dir_all(parent)
            .with_context(|| format!("cannot create output directory {}", parent.display()))?;
    }
    Ok(())
}

/// Windows-native path through the shared Media Foundation encoder.
///
/// Declared colour range: the encoder consumes BT.601 full-range NV12
/// (`Frame::to_nv12`), while the ffmpeg path encodes BT.601 limited range by
/// default — the two paths are not byte-identical by design.
fn render_mp4_media_foundation(plan: &RenderPlan, output: &Path, options: &Mp4Options) -> Result<()> {
    ensure_output_parent(output)?;
    let duration = plan_duration(plan, options);
    let fps = plan.settings.frame_rate.max(1);
    let sample_rate = plan.settings.sample_rate.max(1);
    let width = plan.settings.width;
    let height = plan.settings.height;
    let bitrate = (width * height * fps / 2).clamp(2_000_000, 20_000_000);
    let mut encoder = Encoder::open(output, width, height, fps, bitrate, sample_rate, true)
        .with_context(|| {
            format!(
                "Media Foundation MP4 encoder failed to open {}; set MUSE_MP4_ENCODER=ffmpeg to use the host ffmpeg path",
                output.display()
            )
        })?;

    // Guide audio: a bound WAV decoded to the plan rate, else the placeholder tone.
    let pcm = match options
        .audio_wav
        .as_ref()
        .filter(|path| path.is_file())
    {
        Some(wav_path) => {
            let mut media = MediaLibrary::new();
            let frames = media
                .audio_pcm(
                    &wav_path.to_string_lossy(),
                    sample_rate,
                    crate::compose::DECODE_LIMITS,
                )
                .with_context(|| format!("cannot open audio {}", wav_path.display()))?;
            let limit = (duration * sample_rate as f64).round() as usize;
            frames
                .iter()
                .take(limit)
                .flat_map(|[left, right]| {
                    [
                        (f64::from(*left) * 32767.0).clamp(-32768.0, 32767.0) as i16,
                        (f64::from(*right) * 32767.0).clamp(-32768.0, 32767.0) as i16,
                    ]
                })
                .collect::<Vec<i16>>()
        }
        None => placeholder_pcm_stereo(duration, sample_rate),
    };
    let total_samples = pcm.len() / 2;

    let compose = compose_options(options);
    let mut media = MediaLibrary::new();
    let frames = ((duration * fps as f64).ceil() as usize).max(1);
    let mut written_samples = 0usize;
    for index in 0..frames {
        let t = plan.normalized_time_at_frame(index);
        let frame = compose_frame(plan, width, height, t, &compose, &mut media)?;
        encoder
            .write_video(&frame.to_nv12(), index as u32, fps)
            .with_context(|| format!("encode frame {index}"))?;
        // Interleave audio with video in one-second blocks, like the scene renderer.
        if (index + 1) % fps as usize == 0 || index + 1 == frames {
            let end_samples = ((index + 1) * sample_rate as usize).min(total_samples);
            if end_samples > written_samples {
                encoder
                    .write_audio(&pcm[written_samples * 2..end_samples * 2], written_samples as u64)
                    .with_context(|| format!("encode audio at sample {written_samples}"))?;
                written_samples = end_samples;
            }
        }
    }
    encoder.finish().context("finish Media Foundation MP4")?;
    Ok(())
}

/// Explicit host path: frame PNGs through the shared codec, muxed by ffmpeg.
fn render_mp4_ffmpeg(plan: &RenderPlan, output: &Path, options: &Mp4Options) -> Result<()> {
    let ffmpeg = find_ffmpeg()?;
    ensure_output_parent(output)?;

    let duration = plan_duration(plan, options);

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
    let compose = compose_options(options);
    let mut media = MediaLibrary::new();
    for index in 0..frames {
        let t = plan.normalized_time_at_frame(index);
        let frame = compose_frame(
            plan,
            plan.settings.width,
            plan.settings.height,
            t,
            &compose,
            &mut media,
        )?;
        media_codec::write_png(
            &frames_dir.join(format!("frame-{index:04}.png")),
            frame.width,
            frame.height,
            &frame.pixels,
        )
        .with_context(|| format!("cannot write frame {index}"))?;
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
    use super::{Mp4EncoderPath, find_ffmpeg, select_mp4_encoder};
    use std::env;

    #[test]
    fn resolves_ffmpeg_when_present() {
        let _ = find_ffmpeg();
    }

    #[test]
    fn selects_encoder_path_explicitly() {
        unsafe {
            env::set_var("MUSE_MP4_ENCODER", "ffmpeg");
        }
        assert_eq!(select_mp4_encoder().unwrap(), Mp4EncoderPath::Ffmpeg);
        unsafe {
            env::set_var("MUSE_MP4_ENCODER", "mf");
        }
        assert_eq!(select_mp4_encoder().unwrap(), Mp4EncoderPath::MediaFoundation);
        unsafe {
            env::set_var("MUSE_MP4_ENCODER", "nvenc");
        }
        let error = select_mp4_encoder().expect_err("unknown value must fail");
        assert!(error.to_string().contains("MUSE_MP4_ENCODER"), "{error}");
        unsafe {
            env::remove_var("MUSE_MP4_ENCODER");
        }
        // Unset default: Media Foundation on Windows, ffmpeg elsewhere.
        assert_eq!(
            select_mp4_encoder().unwrap(),
            if cfg!(windows) {
                Mp4EncoderPath::MediaFoundation
            } else {
                Mp4EncoderPath::Ffmpeg
            }
        );
    }
}
