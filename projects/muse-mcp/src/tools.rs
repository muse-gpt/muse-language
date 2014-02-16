use crate::{
    server::MuseMcpServer,
    types::{AssetInfoRequest, EditBlockRequest, ReadScriptRequest},
};
use muse_types::RenderSettings;
use rmcp::{
    Json, ServerHandler,
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ContentBlock},
    tool, tool_handler, tool_router,
};
use serde_json::{Value, json};
use std::path::Path;

fn project_path(
    project: &Path,
    relative_path: Option<&str>,
    default: &str,
) -> Result<std::path::PathBuf, CallToolResult> {
    let relative_path = relative_path.unwrap_or(default);
    let candidate = project.join(relative_path);
    if candidate.starts_with(project) {
        Ok(candidate)
    } else {
        Err(CallToolResult::error(vec![ContentBlock::text(
            "path must remain inside the Muse project",
        )]))
    }
}

fn tool_error(error: impl std::fmt::Display) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(error.to_string())])
}

#[tool_router]
impl MuseMcpServer {
    #[tool(
        name = "muse.read_script",
        description = "Read a Muse script and return its compilation diagnostics."
    )]
    fn read_script(&self, Parameters(request): Parameters<ReadScriptRequest>) -> Result<Json<Value>, CallToolResult> {
        let path = project_path(&self.project, request.path.as_deref(), "main.muse")?;
        let source = std::fs::read_to_string(&path).map_err(tool_error)?;
        let compilation = muse_ir::compile(&source, RenderSettings::default());
        Ok(Json(json!({
            "path": path.strip_prefix(&self.project).unwrap_or(&path).display().to_string(),
            "source": source,
            "compilation": compilation,
        })))
    }

    #[tool(
        name = "muse.edit_block",
        description = "Edit a named Muse block. This tool is reserved until an AST-preserving formatter is available."
    )]
    fn edit_block(&self, Parameters(request): Parameters<EditBlockRequest>) -> CallToolResult {
        let _ = (&request.path, &request.block, &request.replacement);
        CallToolResult::error(vec![ContentBlock::text(
            "muse.edit_block is not available yet: editing requires an AST-preserving formatter; generate a precise source patch and validate it with muse.read_script instead",
        )])
    }

    #[tool(
        name = "muse.suggest_skills",
        description = "List local Muse project skill manifests available to an agent."
    )]
    fn suggest_skills(&self) -> Result<Json<Value>, CallToolResult> {
        let skills = muse_project::list_skills(&self.project).map_err(tool_error)?;
        Ok(Json(json!({ "skills": skills })))
    }

    #[tool(
        name = "muse.get_asset_info",
        description = "Inspect media information for an asset in the current Muse project."
    )]
    fn get_asset_info(&self, Parameters(request): Parameters<AssetInfoRequest>) -> Result<Json<Value>, CallToolResult> {
        let path = project_path(&self.project, Some(&request.path), "")?;
        let info = muse_project::media_info(&path).map_err(tool_error)?;
        Ok(Json(json!({
            "path": path.strip_prefix(&self.project).unwrap_or(&path).display().to_string(),
            "info": info,
        })))
    }
}

#[tool_handler(router = Self::tool_router(), name = "muse", instructions = "Use Muse tools to inspect and validate a local Muse project.")]
impl ServerHandler for MuseMcpServer {}
