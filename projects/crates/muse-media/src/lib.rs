//! Render backends and preview output for Muse.
mod backend;
mod compose;
pub mod gpu;
mod lut;
mod preview;
mod wav;

pub use backend::{
    BackendCapability, LocalPreviewBackend, Mp4Options, OutputFormat, RenderBackend, capability,
    find_ffmpeg, render_mp4, render_mp4_with,
};
pub use compose::{
    ComposeOptions, compose_frame, plan_background_rgb, plan_bloom, plan_grain, plan_has_images,
    plan_lut, plan_vignette,
};
pub use lut::{apply_lut_rgb, resolve_lut};
pub use gpu::{
    CompositorKind, CompositorPreference, GpuBackendInfo, GpuStatus, compositor_preference,
    preferred_compositor, probe_gpu,
};
pub use preview::{PreviewManifest, PreviewOptions, render_preview, render_preview_with};
pub use wav::write_wav;
