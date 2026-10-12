//! Render backends and preview output for Muse.
mod backend;
mod preview;
mod wav;

pub use backend::render_mp4;
pub use backend::{BackendCapability, LocalPreviewBackend, OutputFormat, RenderBackend, capability};
pub use preview::{PreviewManifest, PreviewOptions, render_preview, render_preview_with};
pub use wav::write_wav;
