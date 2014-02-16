use anyhow::Result;
use rmcp::ServiceExt;
use std::path::PathBuf;

/// An MCP server scoped to one local Muse project directory.
#[derive(Debug, Clone)]
pub struct MuseMcpServer {
    pub(crate) project: PathBuf,
}

impl MuseMcpServer {
    pub fn new(project: PathBuf) -> Self {
        Self { project }
    }
}

/// Serve Muse MCP over stdio until the client disconnects.
pub async fn serve(project: PathBuf) -> Result<()> {
    let server = MuseMcpServer::new(project).serve(rmcp::transport::stdio()).await?;
    server.waiting().await?;
    Ok(())
}
