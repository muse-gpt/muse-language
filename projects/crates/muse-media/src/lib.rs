//! Render backends and preview output for Muse.
mod backend;
mod compose;
mod preview;
mod wav;

pub use backend::{
    BackendCapability, LocalPreviewBackend, Mp4Options, OutputFormat, RenderBackend, capability, render_mp4,
    render_mp4_with,
};
pub use compose::{ComposeOptions, compose_frame, plan_background_rgb, plan_has_images};
pub use preview::{PreviewManifest, PreviewOptions, render_preview, render_preview_with};
pub use wav::write_wav;
