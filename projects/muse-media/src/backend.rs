use anyhow::{Result, bail};
use muse_ir::RenderPlan;
use std::path::Path;

pub trait RenderBackend {
    fn render(&self, plan: &RenderPlan, output: &Path) -> Result<()>;
}
#[derive(Debug, Default)]
pub struct LocalPreviewBackend;
impl RenderBackend for LocalPreviewBackend {
    fn render(&self, plan: &RenderPlan, output: &Path) -> Result<()> {
        crate::render_preview(plan, output, 0.1).map(|_| ())
    }
}
pub fn render_mp4(_plan: &RenderPlan, _output: &Path) -> Result<()> {
    bail!("AV1-in-MP4 encoding is not enabled in this build; run `muse preview` for pure-Rust PNG/WAV output")
}
