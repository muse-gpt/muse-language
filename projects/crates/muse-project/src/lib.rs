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
mod prefab;
mod types;
pub use prefab::{
    KNOWN_COMPONENT_TYPES, KNOWN_PORT_TYPES, PrefabComponent, PrefabDefinition,
    PrefabNestedInstance, PrefabObject, PrefabPort, check_project_prefabs, discover_prefab_paths,
    load_prefab_von, validate_prefab,
};
pub use types::{
    ArtifactManifest, ArtifactRecord, AssetMeta, AssetSource, ImportSettings, OutputKind,
    OutputSpec, ProjectEntry, ProjectInfo, ProjectManifest, RenderDefaults, RenderOverride,
    SkillManifest,
};

/// Merge project → entry → output render settings (later wins for set fields).
pub fn resolve_render_settings(
    manifest: &ProjectManifest,
    entry: Option<&ProjectEntry>,
    output: Option<&OutputSpec>,
) -> RenderDefaults {
    let mut settings = manifest.render.clone();
    let apply = |settings: &mut RenderDefaults, overlay: &RenderOverride| {
        if let Some(width) = overlay.width {
            settings.width = width;
        }
        if let Some(height) = overlay.height {
            settings.height = height;
        }
        if let Some(frame_rate) = overlay.frame_rate {
            settings.frame_rate = frame_rate;
        }
    };
    if let Some(entry) = entry {
        if let Some(overlay) = &entry.render {
            apply(&mut settings, overlay);
        }
    }
    if let Some(output) = output {
        if let Some(overlay) = &output.render {
            apply(&mut settings, overlay);
        }
    }
    settings
}

/// Resolve the project manifest path: root `muse.von` only (true VON; no `muse.toml`).
pub fn manifest_path(project: &Path) -> Result<std::path::PathBuf> {
    let von = project.join("muse.von");
    if von.is_file() {
        return Ok(von);
    }
    bail!(
        "project manifest not found at {} (expected muse.von; muse.toml is not accepted)",
        von.display()
    )
}

/// 读取并校验工程清单，确保入口和产物 id 在工程内唯一且路径不为空。
///
/// `muse.von` is **true VON** (`oak-von`). TOML content or a `muse.toml` file is rejected.
pub fn load_manifest(project: &Path) -> Result<ProjectManifest> {
    let path = manifest_path(project)?;
    let raw =
        fs::read_to_string(&path).with_context(|| format!("cannot read {}", path.display()))?;
    let manifest: ProjectManifest = oak_von::from_str(&raw).map_err(|err| {
        anyhow::anyhow!("invalid VON project manifest {}: {err}", path.display())
    })?;
    validate_manifest(&manifest)?;
    Ok(manifest)
}

pub fn validate_manifest(manifest: &ProjectManifest) -> Result<()> {
    if manifest.project.id.trim().is_empty() || manifest.project.version.trim().is_empty() {
        bail!("project id and version are required");
    }
    let mut entry_ids = std::collections::HashSet::new();
    for entry in &manifest.entries {
        if entry.id.trim().is_empty() || entry.source.trim().is_empty() {
            bail!("entry id and source are required");
        }
        if !entry_ids.insert(&entry.id) {
            bail!("duplicate entry id `{}`", entry.id);
        }
        let mut output_ids = std::collections::HashSet::new();
        for output in &entry.outputs {
            if output.id.trim().is_empty() || output.path.trim().is_empty() {
                bail!("entry `{}` has an output without id or path", entry.id);
            }
            if !output_ids.insert(&output.id) {
                bail!("entry `{}` has duplicate output id `{}`", entry.id, output.id);
            }
        }
    }
    Ok(())
}
pub fn init(path: &Path) -> Result<()> {
    fs::create_dir_all(path.join("assets").join("scenes"))?;
    fs::create_dir_all(path.join("assets").join("audio"))?;
    fs::create_dir_all(path.join("assets").join("textures"))?;
    fs::create_dir_all(path.join("library"))?;
    fs::write(
        path.join("muse.von"),
        r#"{
  project: {
    id: "muse-project"
    version: "0.1.0"
  }
  render: {
    width: 1920
    height: 1080
    frame_rate: 24
  }
  entries: [
    {
      id: "main"
      source: "assets/scenes/main.muse"
      outputs: [
        { id: "preview", kind: "preview", path: "library/main/preview" }
      ]
    }
  ]
}
"#,
    )?;
    fs::write(path.join("assets").join("scenes").join("main.muse"), SAMPLE)?;
    fs::write(
        path.join("assets").join("scenes").join("main.muse.meta"),
        "guid: muse-scene-main\nversion: 1\nkind: scene\n",
    )?;
    Ok(())
}

/// Resolve an entry by id, or the sole entry when `entry_id` is `None`.
pub fn find_entry<'a>(manifest: &'a ProjectManifest, entry_id: Option<&str>) -> Result<&'a ProjectEntry> {
    match entry_id {
        Some(id) => manifest
            .entries
            .iter()
            .find(|entry| entry.id == id)
            .with_context(|| format!("unknown entry `{id}`")),
        None if manifest.entries.len() == 1 => Ok(&manifest.entries[0]),
        None if manifest.entries.is_empty() => bail!("project has no entries"),
        None => bail!("multiple entries; pass --entry <id>"),
    }
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
    if kind == MediaKind::Video {
        bail!(
            "video asset `{}` is not supported by the current pure-Rust compositor; use PNG/JPEG/WebP image layers and WAV/FLAC/MP3 audio, or retain the source as an external reference",
            path.display()
        )
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::{env, fs, time::{SystemTime, UNIX_EPOCH}};

    #[test]
    fn validates_multiple_outputs() {
        let m: ProjectManifest = oak_von::from_str(
            r#"{
  project: { id: "p", version: "0.1" }
  entries: [
    {
      id: "e"
      source: "e.muse"
      outputs: [ { id: "a", kind: "audio", path: "a.wav" } ]
    }
  ]
}"#,
        )
        .unwrap();
        validate_manifest(&m).unwrap();
    }

    #[test]
    fn merges_entry_and_output_render_overrides() {
        let m: ProjectManifest = oak_von::from_str(
            r#"{
  project: { id: "p", version: "0.1" }
  render: { width: 1920, height: 1080, frame_rate: 24 }
  entries: [
    {
      id: "vertical"
      source: "v.muse"
      render: { width: 1080, height: 1920 }
      outputs: [
        { id: "video", kind: "video", path: "out.mp4", render: { frame_rate: 30 } }
      ]
    }
  ]
}"#,
        )
        .unwrap();
        validate_manifest(&m).unwrap();
        let entry = &m.entries[0];
        let output = &entry.outputs[0];
        let settings = resolve_render_settings(&m, Some(entry), Some(output));
        assert_eq!(settings.width, 1080);
        assert_eq!(settings.height, 1920);
        assert_eq!(settings.frame_rate, 30);
    }

    #[test]
    fn rejects_toml_shaped_muse_von() {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = env::temp_dir().join(format!("muse-project-toml-von-{stamp}"));
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("muse.von"),
            r#"[project]
id = "p"
version = "0.1"
[[entries]]
id = "e"
source = "e.muse"
"#,
        )
        .unwrap();
        let err = load_manifest(&root).unwrap_err().to_string();
        assert!(
            err.contains("invalid VON") || err.to_lowercase().contains("von"),
            "{err}"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn prefers_muse_von_and_scaffolds_unity_tree() {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = env::temp_dir().join(format!("muse-project-init-{stamp}"));
        init(&root).unwrap();
        assert!(root.join("muse.von").is_file());
        assert!(!root.join("muse.toml").exists());
        assert!(root.join("assets/scenes/main.muse").is_file());
        assert!(root.join("library").is_dir());
        let manifest = load_manifest(&root).unwrap();
        assert_eq!(manifest.entries[0].source, "assets/scenes/main.muse");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_muse_toml_even_when_present() {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = env::temp_dir().join(format!("muse-project-toml-only-{stamp}"));
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("muse.toml"),
            r#"[project]
id = "p"
version = "0.1"
[[entries]]
id = "e"
source = "e.muse"
"#,
        )
        .unwrap();
        let err = load_manifest(&root).unwrap_err().to_string();
        assert!(
            err.contains("muse.von") && err.to_lowercase().contains("toml"),
            "{err}"
        );
        let _ = fs::remove_dir_all(root);
    }
}
