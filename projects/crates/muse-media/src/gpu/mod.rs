//! Compositor status reporting.
//!
//! The wgpu backend was retired (contract 14 §1.2: production paths must not
//! depend on third-party GPU runtimes). Muse composes every frame through the
//! CPU reference path; these types exist so hosts can still report the
//! requested preference and why it degraded.

mod probe;

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
