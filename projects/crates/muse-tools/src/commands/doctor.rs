use anyhow::Result;
use muse_media::{GpuStatus, find_ffmpeg, probe_gpu};
use serde::Serialize;

use crate::app::DoctorArgs;

#[derive(Debug, Serialize)]
struct DoctorReport {
    ok: bool,
    compositor: String,
    gpu: muse_media::GpuBackendInfo,
    ffmpeg: DoctorCheck,
}

#[derive(Debug, Serialize)]
struct DoctorCheck {
    ok: bool,
    message: String,
}

pub(crate) fn doctor(args: &DoctorArgs) -> Result<()> {
    let gpu = probe_gpu();
    let ffmpeg = match find_ffmpeg() {
        Ok(path) => DoctorCheck {
            ok: true,
            message: format!("ffmpeg for encode: {}", path.display()),
        },
        Err(error) => DoctorCheck {
            ok: false,
            message: format!("{error}"),
        },
    };

    // ffmpeg is optional (encode-only). GPU forced-unavailable is a hard fail when preferred.
    let ok = match gpu.status {
        GpuStatus::Available | GpuStatus::Disabled => true,
        GpuStatus::Unavailable => gpu.preferred.as_str() != "gpu",
    };

    let report = DoctorReport {
        ok,
        compositor: gpu.preferred.as_str().into(),
        gpu,
        ffmpeg,
    };

    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!(
            "muse doctor: {}",
            if report.ok { "ok" } else { "issues found" }
        );
        println!("  compositor: {}", report.compositor);
        println!(
            "  {}: gpu: {}",
            status_label(report.gpu.status != GpuStatus::Unavailable || report.ok),
            report.gpu.message
        );
        if let Some(name) = report.gpu.adapter_name.as_deref() {
            println!(
                "         adapter: {} ({}/{})",
                name,
                report.gpu.backend.as_deref().unwrap_or("?"),
                report.gpu.device_type.as_deref().unwrap_or("?")
            );
        }
        println!(
            "  {}: ffmpeg: {}",
            status_label(report.ffmpeg.ok),
            report.ffmpeg.message
        );
        println!("         note: ffmpeg is encode-only; GPU is the compositor");
    }

    if report.ok {
        Ok(())
    } else {
        anyhow::bail!("doctor found issues")
    }
}

fn status_label(ok: bool) -> &'static str {
    if ok { "ok" } else { "FAIL" }
}
