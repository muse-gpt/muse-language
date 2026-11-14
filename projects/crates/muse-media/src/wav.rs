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
/// 归属裁决（G7-W4）：混音语义归 `media.audio::build_audio`（stereo、帧基
/// `TimeBase`、fade 字段、required 合同槽）。本函数只做 Muse 工程语义的
/// lowering：秒→帧换算、uri→路径解析与「绑定轨缺失是工程合同错误」的
/// 显式前置检查（轨道 id 只有 Muse 语言层知道）。
pub fn mix_plan_audio(
    plan: &RenderPlan,
    out: &Path,
    seconds: f64,
    asset_root: &Path,
) -> Result<MixedAudioSource> {
    let rate = plan.settings.sample_rate.max(1);
    let fps = plan.settings.frame_rate.max(1);
    let seconds = seconds.max(0.0);
    if plan.audio_tracks.is_empty() {
        write_placeholder_tone_wav(out, seconds, rate)?;
        return Ok(MixedAudioSource::PlaceholderTone);
    }
    let duration_frames = (seconds * fps as f64).round().max(0.0) as u32;
    let mut clips = Vec::with_capacity(plan.audio_tracks.len());
    for track in &plan.audio_tracks {
        let path = resolve_uri(asset_root, &track.uri);
        if !path.is_file() {
            bail!("audio track `{}` missing file {}", track.id, path.display());
        }
        let to_frames = |t: f64| (t.max(0.0) * fps as f64).round() as u32;
        let duration_frames = track
            .file_end
            .map(|end| to_frames((end - track.file_start).max(0.0)))
            .filter(|d| *d > 0);
        clips.push(media_audio::ClipPlacement {
            src: path.to_string_lossy().into_owned(),
            at_frames: to_frames(track.timeline_offset),
            duration_frames,
            offset_frames: to_frames(track.file_start),
            volume: track.gain_linear as f32,
            fade_in_frames: to_frames(track.fade_in),
            fade_out_frames: to_frames(track.fade_out),
            // Muse 绑定轨是工程声明合同：解码失败必须显式报错。
            required: true,
        });
    }
    let audio_plan = media_audio::AudioPlan {
        sample_rate: rate,
        channels: 2,
        time_base: media_types::TimeBase::from_fps(fps),
        duration_frames,
        bgm: None,
        narration: None,
        sfx: Vec::new(),
        voice: Vec::new(),
        notes: Vec::new(),
        patterns: Vec::new(),
        clips,
    };
    let mut media = MediaLibrary::new();
    let samples = media_audio::build_audio(&audio_plan, &mut media, DECODE_LIMITS)
        .with_context(|| "media.audio mix failed")?;
    write_stereo_i16(out, &samples, rate)?;
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
        // G7-W4 验收：采样数/声道/头与声明窗口一致（帧基 TimeBase 推导），
        // 绑定轨内容非静音（独立解码回读，不与实现共享 oracle 路径）。
        let bytes = fs::read(&out).unwrap();
        let wav = media_codec::parse_wav_pcm16(&bytes).expect("mix output is pcm16");
        assert_eq!(wav.sample_rate, 48_000);
        assert_eq!(wav.frames.len(), (0.25_f64 * 48_000_f64).round() as usize);
        let peak = wav
            .frames
            .iter()
            .flat_map(|f| [f[0], f[1]])
            .fold(0.0f32, |a, s| a.max(s.abs()));
        assert!(peak > 0.01, "bound track must not mix to silence, peak {peak}");
        assert!(
            wav.frames.iter().all(|f| f[0].is_finite() && f[1].is_finite()),
            "all samples finite"
        );
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
