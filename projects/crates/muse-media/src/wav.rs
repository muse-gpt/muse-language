//! PCM WAV helpers: placeholder tone and bound-track mix, written and decoded
//! through the shared host codec (stereo PCM16 output, explicit sample rate).
use anyhow::{Context, Result, bail};
use media_codec::MediaLibrary;
use muse_ir::RenderPlan;
use std::{fs, path::Path};

use crate::compose::DECODE_LIMITS;

/// Development placeholder: fixed 440 Hz tone. Prefer [`mix_plan_audio`] when tracks exist.
pub fn write_wav(path: &Path, seconds: f64, rate: u32) -> Result<()> {
    write_placeholder_tone_wav(path, seconds, rate)
}

pub fn write_placeholder_tone_wav(path: &Path, seconds: f64, rate: u32) -> Result<()> {
    let samples = placeholder_pcm_stereo(seconds, rate);
    write_stereo_i16(path, &samples, rate)
}

/// Stereo PCM16 placeholder tone samples (440 Hz) for in-memory muxing.
pub(crate) fn placeholder_pcm_stereo(seconds: f64, rate: u32) -> Vec<i16> {
    let count = (seconds.max(0.0) * rate as f64).round() as u32;
    let mut samples = Vec::with_capacity(count as usize * 2);
    for index in 0..count {
        let sample =
            ((index as f64 * 440.0 * std::f64::consts::TAU / rate as f64).sin() * 1600.0) as i16;
        // The shared codec writes interleaved stereo; the mono tone is widened.
        samples.push(sample);
        samples.push(sample);
    }
    samples
}

/// Mix result metadata for manifests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MixedAudioSource {
    Tracks,
    PlaceholderTone,
}

/// Write plan audio for wall-clock window `[0, seconds)`.
///
/// Uses bound tracks when present; otherwise writes the placeholder tone.
/// Does not invent score synthesis here (orthogonal: composeScore is separate).
pub fn mix_plan_audio(
    plan: &RenderPlan,
    out: &Path,
    seconds: f64,
    asset_root: &Path,
) -> Result<MixedAudioSource> {
    let rate = plan.settings.sample_rate.max(1);
    let seconds = seconds.max(0.0);
    if plan.audio_tracks.is_empty() {
        write_placeholder_tone_wav(out, seconds, rate)?;
        return Ok(MixedAudioSource::PlaceholderTone);
    }
    let sample_count = (seconds * rate as f64).round().max(0.0) as usize;
    let mut mix = vec![0.0f64; sample_count];
    let mut media = MediaLibrary::new();

    for track in &plan.audio_tracks {
        let path = resolve_uri(asset_root, &track.uri);
        if !path.is_file() {
            bail!("audio track `{}` missing file {}", track.id, path.display());
        }
        let samples = read_mono_f64(&mut media, &path, rate)?;
        let file_start = (track.file_start.max(0.0) * rate as f64).round() as usize;
        let file_end = track
            .file_end
            .map(|t| (t.max(0.0) * rate as f64).round() as usize)
            .unwrap_or(samples.len())
            .min(samples.len());
        if file_end <= file_start {
            continue;
        }
        let clip = &samples[file_start..file_end];
        let timeline_start = (track.timeline_offset.max(0.0) * rate as f64).round() as isize;
        let fade_in = (track.fade_in.max(0.0) * rate as f64).round() as usize;
        let fade_out = (track.fade_out.max(0.0) * rate as f64).round() as usize;
        let gain = track.gain_linear;

        for (i, &s) in clip.iter().enumerate() {
            let out_i = timeline_start + i as isize;
            if out_i < 0 || out_i as usize >= sample_count {
                continue;
            }
            let mut g = gain;
            if fade_in > 0 && i < fade_in {
                g *= i as f64 / fade_in as f64;
            }
            let from_end = clip.len().saturating_sub(1).saturating_sub(i);
            if fade_out > 0 && from_end < fade_out {
                g *= (from_end + 1) as f64 / fade_out as f64;
            }
            mix[out_i as usize] += s * g;
        }
    }

    write_stereo_from_f64(out, rate, &mix)?;
    Ok(MixedAudioSource::Tracks)
}

fn resolve_uri(asset_root: &Path, uri: &str) -> std::path::PathBuf {
    let path = Path::new(uri);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        asset_root.join(path)
    }
}

/// Decode one audio file to mono f64 at the plan rate.
///
/// The rate is an explicit input contract of the host decoder (it resamples
/// while decoding), so no private resampler sits between import and mix.
fn read_mono_f64(media: &mut MediaLibrary, path: &Path, target_rate: u32) -> Result<Vec<f64>> {
    let frames = media
        .audio_pcm(&path.to_string_lossy(), target_rate, DECODE_LIMITS)
        .with_context(|| format!("open {}", path.display()))?;
    Ok(frames
        .iter()
        .map(|[left, right]| (f64::from(*left) + f64::from(*right)) / 2.0)
        .collect())
}

fn write_stereo_i16(path: &Path, samples: &[i16], rate: u32) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let raw = path
        .to_str()
        .with_context(|| format!("path is not valid UTF-8: {}", path.display()))?;
    media_codec::write_wav(raw, samples, rate)
        .with_context(|| format!("create {}", path.display()))?;
    Ok(())
}

/// Quantize the mono f64 mix to interleaved stereo PCM16 (same scale and
/// clamp as the historical writer, with the channels widened).
fn write_stereo_from_f64(path: &Path, rate: u32, samples: &[f64]) -> Result<()> {
    let mut interleaved = Vec::with_capacity(samples.len() * 2);
    for &sample in samples {
        let clamped = (sample * 32767.0).clamp(-32768.0, 32767.0) as i16;
        interleaved.push(clamped);
        interleaved.push(clamped);
    }
    write_stereo_i16(path, &interleaved, rate)
}

#[cfg(test)]
mod tests {
    use super::*;
    use muse_ir::{AudioRole, AudioSourceKind, AudioTrack, TimeMap};
    use std::env;

    #[test]
    fn placeholder_tone_writes_stereo_pcm16_with_declared_rate() {
        let dir = env::temp_dir().join(format!("muse-tone-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("tone.wav");
        write_placeholder_tone_wav(&path, 0.01, 44_100).unwrap();
        let bytes = fs::read(&path).unwrap();
        let wav = media_codec::parse_wav_pcm16(&bytes).expect("pcm16 decodes");
        assert_eq!(wav.sample_rate, 44_100);
        assert_eq!(wav.frames.len(), (0.01_f64 * 44_100.0).round() as usize);
        for frame in &wav.frames {
            assert_eq!(frame[0], frame[1], "mono tone widens to identical stereo");
        }
        // Same tone formula as the historical writer: index 1 carries the first
        // non-zero sample at the declared rate.
        let expected =
            ((1.0 * 440.0 * std::f64::consts::TAU / 44_100.0).sin() * 1600.0) as i16;
        assert!(wav.frames[1][0].abs() > 0.0);
        assert!((wav.frames[1][0] - f32::from(expected) / 32768.0).abs() < 1e-6);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_bound_track_is_an_explicit_error() {
        let dir = env::temp_dir().join(format!("muse-mix-missing-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let mut settings = muse_ir::compile(
            "timeline { act main : 0s..1s } tempo { bpm 120 }",
            Default::default(),
        )
        .plan
        .expect("plan")
        .settings;
        settings.sample_rate = 48_000;
        let plan = plan_with_track(settings, "absent.wav");
        let error = mix_plan_audio(&plan, &dir.join("out.wav"), 0.25, &dir)
            .expect_err("missing file must fail");
        assert!(
            error.to_string().contains("audio track `master` missing file"),
            "{error}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[cfg(windows)]
    #[test]
    fn mixes_bound_track_without_placeholder() {
        let dir = env::temp_dir().join(format!("muse-mix-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let src = dir.join("bed.wav");
        write_placeholder_tone_wav(&src, 0.25, 48_000).unwrap();

        let mut settings = muse_ir::compile(
            "timeline { act main : 0s..1s } tempo { bpm 120 }",
            Default::default(),
        )
        .plan
        .expect("plan")
        .settings;
        settings.sample_rate = 48_000;

        let plan = plan_with_track(settings, "bed.wav");
        let out = dir.join("out.wav");
        let kind = mix_plan_audio(&plan, &out, 0.25, &dir).unwrap();
        assert_eq!(kind, MixedAudioSource::Tracks);
        assert!(out.is_file());
        let _ = fs::remove_dir_all(&dir);
    }

    fn plan_with_track(settings: muse_types::RenderSettings, uri: &str) -> RenderPlan {
        RenderPlan {
            settings,
            duration_seconds: 0.25,
            time_map: TimeMap::new(120.0, 4),
            acts: vec![],
            shots: vec![],
            assets: vec![],
            audio_source: AudioSourceKind::Tracks,
            audio_tracks: vec![AudioTrack {
                id: "master".into(),
                role: AudioRole::Song,
                uri: uri.into(),
                file_start: 0.0,
                file_end: Some(0.25),
                timeline_offset: 0.0,
                gain_linear: 1.0,
                fade_in: 0.0,
                fade_out: 0.0,
                tail_silence: 0.0,
            }],
            lyric_tracks: vec![],
            lyric_layers: vec![],
            visual_layers: 0,
            visuals: vec![],
            transitions: 0,
            intents: vec![],
            orchestration: vec![],
            prefab_instances: vec![],
        }
    }
}
