use crate::app::McpCommand;
use anyhow::Result;

pub(crate) async fn run(command: McpCommand) -> Result<()> {
    match command {
        McpCommand::Serve { project } => {
            // Keep the transport boundary here: muse-mcp is migrating to an async rmcp server.
            muse_mcp::serve(project).await
        }
    }
}
