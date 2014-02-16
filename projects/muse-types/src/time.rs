use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BeatPosition {
    pub bar: u32,
    pub beat: f64,
}
impl BeatPosition {
    pub const fn from_bar(bar: u32) -> Self {
        Self { bar, beat: 0.0 }
    }
}
