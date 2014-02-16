use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RenderSettings {
    pub width: u32,
    pub height: u32,
    pub frame_rate: u32,
    pub sample_rate: u32,
}
impl Default for RenderSettings {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            frame_rate: 24,
            sample_rate: 48_000,
        }
    }
}
