//! Render backends and frame composition for Muse.
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
    ComposeOptions, compose_frame, plan_background_rgb, plan_bloom, plan_grain, plan_has_images,
    plan_lut, plan_vignette,
};
pub use lut::{apply_lut_rgb, resolve_lut};
pub use gpu::{
    CompositorKind, CompositorPreference, GpuBackendInfo, GpuStatus, compositor_preference,
    preferred_compositor, probe_gpu,
};
pub use wav::{
    MixedAudioSource, mix_plan_audio, write_placeholder_tone_wav, write_wav,
};
