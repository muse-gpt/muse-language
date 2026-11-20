use super::CompositorKind;
use serde::Serialize;
use std::env;

#[derive(Debug, Clone, Serialize)]
pub struct GpuBackendInfo {
    pub status: GpuStatus,
    pub preferred: CompositorKind,
    pub adapter_name: Option<String>,
    pub backend: Option<String>,
    pub device_type: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GpuStatus {
    Available,
    Unavailable,
    Disabled,
}

/// `MUSE_COMPOSITOR=gpu|cpu|auto` (default `auto`). Parsed so hosts can report
/// why every frame is composed on the CPU reference path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompositorPreference {
    Auto,
    Gpu,
    Cpu,
}

pub fn compositor_preference() -> CompositorPreference {
    match env::var("MUSE_COMPOSITOR")
        .unwrap_or_else(|_| "auto".into())
        .to_ascii_lowercase()
        .as_str()
    {
        "cpu" => CompositorPreference::Cpu,
        "gpu" => CompositorPreference::Gpu,
        _ => CompositorPreference::Auto,
    }
}

/// Resolve effective compositor after preference + probe.
pub fn preferred_compositor() -> CompositorKind {
    match compositor_preference() {
        CompositorPreference::Cpu => CompositorKind::Cpu,
        CompositorPreference::Gpu => CompositorKind::Gpu,
        CompositorPreference::Auto => CompositorKind::Cpu,
    }
}

pub fn probe_gpu() -> GpuBackendInfo {
    match compositor_preference() {
        CompositorPreference::Cpu => GpuBackendInfo {
            status: GpuStatus::Disabled,
            preferred: CompositorKind::Cpu,
            adapter_name: None,
            backend: None,
            device_type: None,
            message: "GPU compositor disabled via MUSE_COMPOSITOR=cpu".into(),
        },
        CompositorPreference::Gpu | CompositorPreference::Auto => GpuBackendInfo {
            status: GpuStatus::Disabled,
            preferred: CompositorKind::Cpu,
            adapter_name: None,
            backend: None,
            device_type: None,
            message: "gpu retired: media-gpu D3D12 backend not yet wired for muse effects".into(),
        },
    }
}
