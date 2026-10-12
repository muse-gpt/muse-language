//! GPU compositor backend (ADR 0005). Formal path for Muse frame composition.
//!
//! ffmpeg remains encode-only / early smoke — not a layer compositor.

#[cfg(feature = "gpu")]
mod compositor;
mod probe;

#[cfg(feature = "gpu")]
pub use compositor::{GpuCompositor, GpuComposeOptions};
pub use probe::{
    CompositorPreference, GpuBackendInfo, GpuStatus, compositor_preference, preferred_compositor,
    probe_gpu,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CompositorKind {
    Gpu,
    Cpu,
}

impl CompositorKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Gpu => "gpu",
            Self::Cpu => "cpu",
        }
    }
}
