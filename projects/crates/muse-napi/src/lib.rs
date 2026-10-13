//! Orthogonal Node-API primitives for Muse.
//!
//! This crate does **not** export CLI-shaped workflows (`check` / `preview` / `render`).
//! Compose those in `@muse-gpt/muse` (TypeScript) from these functions.
#![deny(clippy::all)]

use std::{collections::HashMap, path::PathBuf};

use muse_ir::{Compilation, RenderPlan, compilation_schema, compile};
use muse_media::{
    ComposeOptions, MixedAudioSource, compose_frame, find_ffmpeg, mix_plan_audio, probe_gpu,
    write_placeholder_tone_wav as media_write_placeholder_tone_wav,
};
use muse_parser::parse;
use muse_project::{find_entry, init as project_init, load_manifest, validate_manifest};
use muse_types::RenderSettings;
use napi::bindgen_prelude::*;
use napi_derive::napi;
use serde::Serialize;
use serde_json::Value;

fn to_napi_err(err: impl std::fmt::Display) -> Error {
    Error::from_reason(err.to_string())
}

fn json_value<T: Serialize>(value: &T) -> Result<Value> {
    serde_json::to_value(value).map_err(to_napi_err)
}

/// Parse Muse source. Returns diagnostics and optional AST JSON (no IR lowering).
#[napi(object)]
pub struct ParseResult {
    pub diagnostics: Value,
    pub script: Option<Value>,
    pub ok: bool,
}

#[napi]
pub fn parse_source(source: String) -> Result<ParseResult> {
    let parsed = parse(&source);
    let ok = !parsed
        .diagnostics
        .iter()
        .any(|d| matches!(d.severity, muse_types::Severity::Error));
    Ok(ParseResult {
        diagnostics: json_value(&parsed.diagnostics)?,
        script: Some(json_value(&parsed.script)?),
        ok,
    })
}

/// Compile Muse source to a render plan (+ diagnostics).
#[napi(object)]
pub struct CompileResult {
    pub diagnostics: Value,
    pub plan: Option<Value>,
    pub ok: bool,
}

#[napi(object)]
pub struct RenderSettingsInput {
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub frame_rate: Option<u32>,
    pub sample_rate: Option<u32>,
}

fn settings_from_input(input: Option<RenderSettingsInput>) -> RenderSettings {
    let mut settings = RenderSettings::default();
    if let Some(input) = input {
        if let Some(width) = input.width {
            settings.width = width;
        }
        if let Some(height) = input.height {
            settings.height = height;
        }
        if let Some(frame_rate) = input.frame_rate {
            settings.frame_rate = frame_rate;
        }
        if let Some(sample_rate) = input.sample_rate {
            settings.sample_rate = sample_rate;
        }
    }
    settings
}

#[napi]
pub fn compile_source(source: String, settings: Option<RenderSettingsInput>) -> Result<CompileResult> {
    let compilation: Compilation = compile(&source, settings_from_input(settings));
    Ok(CompileResult {
        ok: compilation.is_success(),
        diagnostics: json_value(&compilation.diagnostics)?,
        plan: compilation.plan.as_ref().map(json_value).transpose()?,
    })
}

/// JSON Schema for the compilation / plan document.
#[napi]
pub fn compilation_schema_json() -> Result<Value> {
    let schema = compilation_schema();
    serde_json::to_value(schema).map_err(to_napi_err)
}

/// Load `muse.von` from a project root.
#[napi]
pub fn load_project_manifest(project_path: String) -> Result<Value> {
    let manifest = load_manifest(PathBuf::from(project_path).as_path()).map_err(to_napi_err)?;
    json_value(&manifest)
}

/// Validate a project manifest object.
#[napi]
pub fn validate_project_manifest(manifest: Value) -> Result<()> {
    let manifest: muse_project::ProjectManifest =
        serde_json::from_value(manifest).map_err(to_napi_err)?;
    validate_manifest(&manifest).map_err(to_napi_err)
}

/// Resolve an entry from a manifest JSON object.
#[napi]
pub fn find_project_entry(manifest: Value, entry_id: Option<String>) -> Result<Value> {
    let manifest: muse_project::ProjectManifest =
        serde_json::from_value(manifest).map_err(to_napi_err)?;
    let entry = find_entry(&manifest, entry_id.as_deref()).map_err(to_napi_err)?;
    json_value(entry)
}

/// Create a starter Muse project tree (`muse.von` + sample scene).
#[napi]
pub fn init_project(path: String) -> Result<()> {
    project_init(PathBuf::from(path).as_path()).map_err(to_napi_err)
}

/// Probe GPU compositor backend status.
#[napi]
pub fn probe_gpu_status() -> Result<Value> {
    json_value(&probe_gpu())
}

/// Locate ffmpeg on PATH (or `MUSE_FFMPEG`), if any.
#[napi]
pub fn find_ffmpeg_path() -> Result<Option<String>> {
    match find_ffmpeg() {
        Ok(path) => Ok(Some(path.to_string_lossy().into_owned())),
        Err(_) => Ok(None),
    }
}

/// Compose one RGB frame from a plan JSON at normalized time `t` in `[0, 1]`.
/// Returns PNG bytes.
#[napi]
pub fn compose_frame_png(
    plan: Value,
    width: u32,
    height: u32,
    t: f64,
    asset_root: Option<String>,
) -> Result<Buffer> {
    let plan: RenderPlan = serde_json::from_value(plan).map_err(to_napi_err)?;
    let options = ComposeOptions {
        asset_root: PathBuf::from(asset_root.unwrap_or_else(|| ".".into())),
    };
    let mut cache = HashMap::new();
    let image = compose_frame(&plan, width, height, t as f32, &options, &mut cache)
        .map_err(to_napi_err)?;
    let mut png = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut png);
    image
        .write_to(&mut cursor, image::ImageFormat::Png)
        .map_err(to_napi_err)?;
    Ok(Buffer::from(png))
}

/// Write the development placeholder tone WAV (440 Hz). Prefer [`mix_plan_audio_wav`] when tracks exist.
#[napi]
pub fn write_placeholder_tone_wav(path: String, seconds: f64, sample_rate: u32) -> Result<()> {
    media_write_placeholder_tone_wav(PathBuf::from(path).as_path(), seconds, sample_rate)
        .map_err(to_napi_err)
}

/// Mix plan-bound audio tracks into a WAV for wall-clock `[0, seconds)`.
/// Returns `"tracks"` or `"placeholder_tone"`.
#[napi]
pub fn mix_plan_audio_wav(
    plan: Value,
    out_path: String,
    seconds: f64,
    asset_root: Option<String>,
) -> Result<String> {
    let plan: RenderPlan = serde_json::from_value(plan).map_err(to_napi_err)?;
    let root = PathBuf::from(asset_root.unwrap_or_else(|| ".".into()));
    let kind = mix_plan_audio(&plan, PathBuf::from(out_path).as_path(), seconds, &root)
        .map_err(to_napi_err)?;
    Ok(match kind {
        MixedAudioSource::Tracks => "tracks".into(),
        MixedAudioSource::PlaceholderTone => "placeholder_tone".into(),
    })
}
