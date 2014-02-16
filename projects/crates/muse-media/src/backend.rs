use anyhow::{Result, bail};
use muse_ir::RenderPlan;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Preview,
    Wav,
    Mp4Av1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendCapability {
    Supported,
    Unsupported,
}

pub const fn capability(format: OutputFormat) -> BackendCapability {
    match format {
        OutputFormat::Preview | OutputFormat::Wav => BackendCapability::Supported,
        OutputFormat::Mp4Av1 => BackendCapability::Unsupported,
    }
}

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
    debug_assert_eq!(capability(OutputFormat::Mp4Av1), BackendCapability::Unsupported);
    bail!(
        "AV1-in-MP4 encoding is not enabled in this pure-Rust build; run `muse preview` for PNG/WAV output or export audio/score formats"
    )
}

#[cfg(test)]
mod tests {
    use super::{BackendCapability, OutputFormat, capability};

    #[test]
    fn exposes_only_verified_pure_rust_outputs() {
        assert_eq!(capability(OutputFormat::Preview), BackendCapability::Supported);
        assert_eq!(capability(OutputFormat::Wav), BackendCapability::Supported);
        assert_eq!(capability(OutputFormat::Mp4Av1), BackendCapability::Unsupported);
    }
}
