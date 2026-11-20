//! Render backends and frame composition for Muse.
//! wgpu 25's `Send`/`Sync` chains are deeper than the default auto-trait
//! recursion limit, which would leave the GPU compositor cache unprovable.
#![recursion_limit = "256"]
mod backend;
mod compose;
mod font_stamp;
pub mod gpu;
mod lut;
mod wav;

pub use backend::{
    Mp4EncoderPath, Mp4Options, find_ffmpeg, mp4_encoder_status, render_mp4, render_mp4_with,
    select_mp4_encoder,
};
pub use compose::{
    ComposeOptions, ComposeReport, compose_frame, compose_report, plan_background_rgb, plan_bloom,
    plan_grain, plan_has_images,
    plan_lut, plan_vignette,
};
pub use lut::{grade_map_strip, resolve_lut};
pub use gpu::{
    CompositorKind, CompositorPreference, GpuBackendInfo, GpuStatus, compositor_preference,
    preferred_compositor, probe_gpu,
};
pub use wav::{
    MixedAudioSource, mix_plan_audio, write_placeholder_tone_wav, write_wav,
};
