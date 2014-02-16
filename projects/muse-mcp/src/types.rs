use rmcp::schemars::JsonSchema;
use serde::Deserialize;

/// Parameters for `muse.read_script`. Paths are relative to the project root.
#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct ReadScriptRequest {
    #[schemars(description = "Muse script path relative to the project root. Defaults to main.muse.")]
    pub path: Option<String>,
}

/// Parameters for `muse.edit_block`. The operation is intentionally not yet implemented.
#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct EditBlockRequest {
    #[schemars(description = "Muse script path relative to the project root. Defaults to main.muse.")]
    pub path: Option<String>,
    #[schemars(description = "Stable block identifier that should be edited.")]
    pub block: String,
    #[schemars(description = "Replacement Muse source for the selected block.")]
    pub replacement: String,
}

/// Parameters for `muse.get_asset_info`. Paths are relative to the project root.
#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct AssetInfoRequest {
    #[schemars(description = "Asset path relative to the project root.")]
    pub path: String,
}
