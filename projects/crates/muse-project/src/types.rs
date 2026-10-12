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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OutputKind {
    Preview,
    Video,
    Audio,
    Score,
    Data,
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
