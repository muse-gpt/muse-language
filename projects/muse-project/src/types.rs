use serde::{Deserialize, Serialize};

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
