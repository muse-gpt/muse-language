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

/// `MUSE_COMPOSITOR=gpu|cpu|auto` (default `auto`).
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
        CompositorPreference::Auto => match probe_gpu().status {
            GpuStatus::Available => CompositorKind::Gpu,
            _ => CompositorKind::Cpu,
        },
    }
}

#[cfg(feature = "gpu")]
pub fn probe_gpu() -> GpuBackendInfo {
    match compositor_preference() {
        CompositorPreference::Cpu => {
            return GpuBackendInfo {
                status: GpuStatus::Disabled,
                preferred: CompositorKind::Cpu,
                adapter_name: None,
                backend: None,
                device_type: None,
                message: "GPU compositor disabled via MUSE_COMPOSITOR=cpu".into(),
            };
        }
        CompositorPreference::Gpu | CompositorPreference::Auto => {}
    }

    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::PRIMARY,
        ..Default::default()
    });
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: false,
    }));

    match adapter {
        Ok(adapter) => {
            let info = adapter.get_info();
            let preferred = match compositor_preference() {
                CompositorPreference::Cpu => CompositorKind::Cpu,
                CompositorPreference::Gpu => CompositorKind::Gpu,
                CompositorPreference::Auto => CompositorKind::Gpu,
            };
            GpuBackendInfo {
                status: GpuStatus::Available,
                preferred,
                adapter_name: Some(info.name),
                backend: Some(format!("{:?}", info.backend)),
                device_type: Some(format!("{:?}", info.device_type)),
                message: "wgpu adapter available".into(),
            }
        }
        Err(error) => GpuBackendInfo {
            status: GpuStatus::Unavailable,
            preferred: CompositorKind::Cpu,
            adapter_name: None,
            backend: None,
            device_type: None,
            message: format!("no wgpu adapter: {error}"),
        },
    }
}

#[cfg(not(feature = "gpu"))]
pub fn probe_gpu() -> GpuBackendInfo {
    GpuBackendInfo {
        status: GpuStatus::Disabled,
        preferred: CompositorKind::Cpu,
        adapter_name: None,
        backend: None,
        device_type: None,
        message: "muse-media built without `gpu` feature".into(),
    }
}
