//! PCM WAV helpers: placeholder tone and bound-track mix (doc 16).
use anyhow::{Context, Result, bail};
use hound::{SampleFormat, WavReader, WavSpec, WavWriter};
use muse_ir::RenderPlan;
use std::{fs, path::Path};

/// Development placeholder: fixed 440 Hz tone. Prefer [`mix_plan_audio`] when tracks exist.
pub fn write_wav(path: &Path, seconds: f64, rate: u32) -> Result<()> {
    write_placeholder_tone_wav(path, seconds, rate)
}

pub fn write_placeholder_tone_wav(path: &Path, seconds: f64, rate: u32) -> Result<()> {
    let count = (seconds.max(0.0) * rate as f64).round() as u32;
    let spec = WavSpec {
        channels: 1,
        sample_rate: rate,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };
    let mut writer = WavWriter::create(path, spec).with_context(|| format!("create {}", path.display()))?;
    for index in 0..count {
        let sample =
            ((index as f64 * 440.0 * std::f64::consts::TAU / rate as f64).sin() * 1600.0) as i16;
        writer.write_sample(sample)?;
    }
    writer.finalize()?;
    Ok(())
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

    for track in &plan.audio_tracks {
        let path = resolve_uri(asset_root, &track.uri);
        if !path.is_file() {
            bail!("audio track `{}` missing file {}", track.id, path.display());
        }
        let samples = read_mono_f64(&path, rate)?;
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

    write_mono_f64(out, rate, &mix)?;
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

fn read_mono_f64(path: &Path, target_rate: u32) -> Result<Vec<f64>> {
    let mut reader = WavReader::open(path).with_context(|| format!("open {}", path.display()))?;
    let spec = reader.spec();
    let channels = spec.channels.max(1) as usize;
    let mut mono = Vec::new();
    match spec.sample_format {
        SampleFormat::Int => {
            let max = (1i64 << (spec.bits_per_sample.saturating_sub(1).max(1))) as f64;
            let mut frame = Vec::with_capacity(channels);
            for sample in reader.samples::<i32>() {
                let s = sample? as f64 / max;
                frame.push(s);
                if frame.len() == channels {
                    mono.push(frame.iter().sum::<f64>() / channels as f64);
                    frame.clear();
                }
            }
        }
        SampleFormat::Float => {
            let mut frame = Vec::with_capacity(channels);
            for sample in reader.samples::<f32>() {
                let s = sample? as f64;
                frame.push(s);
                if frame.len() == channels {
                    mono.push(frame.iter().sum::<f64>() / channels as f64);
                    frame.clear();
                }
            }
        }
    }
    if spec.sample_rate == target_rate || mono.is_empty() {
        return Ok(mono);
    }
    Ok(resample_linear(&mono, spec.sample_rate, target_rate))
}

fn resample_linear(input: &[f64], from: u32, to: u32) -> Vec<f64> {
    if from == 0 || to == 0 || input.is_empty() {
        return input.to_vec();
    }
    let out_len = ((input.len() as f64) * f64::from(to) / f64::from(from)).round() as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let src = i as f64 * f64::from(from) / f64::from(to);
        let i0 = src.floor() as usize;
        let i1 = (i0 + 1).min(input.len() - 1);
        let frac = src - i0 as f64;
        out.push(input[i0] * (1.0 - frac) + input[i1] * frac);
    }
    out
}

fn write_mono_f64(path: &Path, rate: u32, samples: &[f64]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let spec = WavSpec {
        channels: 1,
        sample_rate: rate,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };
    let mut writer = WavWriter::create(path, spec).with_context(|| format!("create {}", path.display()))?;
    for &s in samples {
        let clamped = (s * 32767.0).clamp(-32768.0, 32767.0) as i16;
        writer.write_sample(clamped)?;
    }
    writer.finalize()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use muse_ir::{AudioRole, AudioSourceKind, AudioTrack, TimeMap};
    use std::env;

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

        let plan = RenderPlan {
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
                uri: "bed.wav".into(),
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
        };
        let out = dir.join("out.wav");
        let kind = mix_plan_audio(&plan, &out, 0.25, &dir).unwrap();
        assert_eq!(kind, MixedAudioSource::Tracks);
        assert!(out.is_file());
        let _ = fs::remove_dir_all(&dir);
    }
}
