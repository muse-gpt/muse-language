use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectManifest {
    pub project: ProjectInfo,
    #[serde(default)]
    pub render: RenderDefaults,
    #[serde(default)]
    pub entries: Vec<ProjectEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectInfo {
    pub id: String,
    pub version: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderDefaults {
    #[serde(default = "default_width")]
    pub width: u32,
    #[serde(default = "default_height")]
    pub height: u32,
    #[serde(default = "default_frame_rate")]
    pub frame_rate: u32,
}

impl Default for RenderDefaults {
    fn default() -> Self {
        Self {
            width: default_width(),
            height: default_height(),
            frame_rate: default_frame_rate(),
        }
    }
}

/// Partial render override (entry or output). Missing fields inherit from the parent level.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct RenderOverride {
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub frame_rate: Option<u32>,
}

fn default_width() -> u32 {
    1920
}
fn default_height() -> u32 {
    1080
}
fn default_frame_rate() -> u32 {
    24
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectEntry {
    pub id: String,
    pub source: String,
    /// Optional per-entry frame size / fps (e.g. vertical 1080×1920).
    #[serde(default)]
    pub render: Option<RenderOverride>,
    #[serde(default)]
    pub outputs: Vec<OutputSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputSpec {
    pub id: String,
    pub kind: OutputKind,
    pub path: String,
    #[serde(default)]
    pub format: Option<String>,
    /// Optional per-output override (wins over entry / project defaults).
    #[serde(default)]
    pub render: Option<RenderOverride>,
}

/// Output product kind. Serialized as a plain string so oak-von (no enum visitor) can round-trip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutputKind {
    Preview,
    Video,
    Audio,
    Score,
    Data,
}

impl OutputKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Preview => "preview",
            Self::Video => "video",
            Self::Audio => "audio",
            Self::Score => "score",
            Self::Data => "data",
        }
    }
}

impl std::fmt::Display for OutputKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for OutputKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim() {
            "preview" => Ok(Self::Preview),
            "video" => Ok(Self::Video),
            "audio" => Ok(Self::Audio),
            "score" => Ok(Self::Score),
            "data" => Ok(Self::Data),
            other => Err(format!("unknown output kind `{other}`")),
        }
    }
}

impl Serialize for OutputKind {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for OutputKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        raw.parse().map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactManifest {
    pub project_id: String,
    pub entry_id: String,
    pub artifacts: Vec<ArtifactRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactRecord {
    pub output_id: String,
    pub path: String,
    pub kind: OutputKind,
    pub source_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetMeta {
    pub guid: String,
    pub hash: String,
    pub version: u32,
    pub source: AssetSource,
    pub import_settings: ImportSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetSource {
    #[serde(rename = "type")]
    pub source_type: String,
    pub original_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ImportSettings {
    pub start_time: Option<f64>,
    pub duration: Option<f64>,
    pub frame_rate: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillManifest {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub generators: Vec<String>,
}
