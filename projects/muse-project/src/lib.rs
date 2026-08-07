//! Project layout, asset metadata, and local skill discovery.
use anyhow::{Context, Result, bail};
use muse_types::{ContentHash, MediaInfo, MediaKind};
use sha2::{Digest, Sha256};
use std::{fs, path::Path};
use symphonia::core::{
    codecs::CodecParameters,
    formats::{FormatOptions, TrackType, probe::Hint},
    io::MediaSourceStream,
    meta::MetadataOptions,
};
mod types;
pub use types::{AssetMeta, AssetSource, ImportSettings, SkillManifest};
pub fn init(path: &Path) -> Result<()> {
    fs::create_dir_all(path.join("assets"))?;
    fs::create_dir_all(path.join("skills"))?;
    fs::write(
        path.join("muse.toml"),
        "[render]\nwidth = 1920\nheight = 1080\nframe_rate = 24\n",
    )?;
    fs::write(path.join("main.muse"), SAMPLE)?;
    Ok(())
}
pub fn hash_file(path: &Path) -> Result<ContentHash> {
    let bytes = fs::read(path)?;
    Ok(ContentHash(format!("sha256:{:x}", Sha256::digest(bytes))))
}
pub fn import_asset(project: &Path, input: &Path) -> Result<AssetMeta> {
    let filename = input.file_name().context("asset path has no file name")?;
    let dest = project.join("assets").join(filename);
    fs::create_dir_all(dest.parent().unwrap())?;
    if input != dest {
        fs::copy(input, &dest)?;
    }
    let hash = hash_file(&dest)?;
    let guid = hash.0.trim_start_matches("sha256:")[..12].to_owned();
    let meta = AssetMeta {
        guid: guid.clone(),
        hash: hash.0,
        version: 1,
        source: AssetSource {
            source_type: "local".to_owned(),
            original_path: input.display().to_string(),
        },
        import_settings: ImportSettings::default(),
    };
    fs::write(
        dest.with_file_name(format!("{}.meta", filename.to_string_lossy())),
        serde_yaml::to_string(&meta)?,
    )?;
    Ok(meta)
}
pub fn media_info(path: &Path) -> Result<MediaInfo> {
    let ext = path
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let kind = match ext.as_str() {
        "png" | "jpg" | "jpeg" | "webp" => MediaKind::Image,
        "wav" | "mp3" | "flac" | "aac" | "m4a" => MediaKind::Audio,
        "mp4" | "webm" => MediaKind::Video,
        "mid" | "midi" | "xml" | "musicxml" | "mxl" => MediaKind::Score,
        _ => MediaKind::Unknown,
    };
    if kind == MediaKind::Unknown {
        bail!("unsupported asset format `{ext}`")
    }
    let mut info = MediaInfo {
        kind,
        format: ext,
        width: None,
        height: None,
        sample_rate: None,
    };
    if matches!(info.kind, MediaKind::Audio | MediaKind::Video) {
        let file = fs::File::open(path)?;
        let mut hint = Hint::new();
        hint.with_extension(&info.format);
        if let Ok(probed) = symphonia::default::get_probe().probe(
            &hint,
            MediaSourceStream::new(Box::new(file), Default::default()),
            FormatOptions::default(),
            MetadataOptions::default(),
        ) && let Some(track) = probed.default_track(TrackType::Audio)
            && let Some(CodecParameters::Audio(parameters)) = &track.codec_params
        {
            info.sample_rate = parameters.sample_rate;
        }
    }
    Ok(info)
}
pub fn list_skills(project: &Path) -> Result<Vec<SkillManifest>> {
    let dir = project.join("skills");
    if !dir.exists() {
        return Ok(vec![]);
    };
    let mut result = vec![];
    for entry in fs::read_dir(dir)? {
        let path = entry?.path().join("skill.yaml");
        if path.exists() {
            result.push(serde_yaml::from_str(&fs::read_to_string(path)?)?);
        }
    }
    Ok(result)
}
pub const SAMPLE: &str = r##"namespace demo;
timeline {
  act intro : 0..8
  act chorus : 8..16
}
tempo { intro 120 bpm }
asset hero = "muse://asset/hero";
music for chorus { key "C major" harmony { at 0 chord "Am7" } curve tension { 8:0.2, 16:0.9 } }
visual for chorus { background color "#16213e" particles { rate 80 } }
intent { emotion "explosion" intensity 0.9 for chorus }
orchestrate for chorus {
  map "explosion" to { music_tension: 0.9, camera_speed: 1.3 }
  hard_constraints: ["no_flash_white_transition"]
  soft_preferences: [{ rule: "use_negative_space", weight: 0.6 }]
  transition_strategy: { on_energy_peak: "snap_zoom" }
}
"##;
