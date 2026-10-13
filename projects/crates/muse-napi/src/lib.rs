//! Orthogonal Node-API primitives for Muse.
//!
//! This crate does **not** export CLI-shaped workflows (`check` / `preview` / `render`).
//! Compose those in `@muse-gpt/muse` (TypeScript) from these functions.
#![deny(clippy::all)]

use std::{collections::HashMap, path::PathBuf};

use muse_ir::{Compilation, CompileOptions, RenderPlan, compilation_schema, compile_with};
use muse_media::{
    ComposeOptions, MixedAudioSource, Mp4Options, compose_frame, find_ffmpeg, mix_plan_audio,
    probe_gpu, render_mp4_with, write_placeholder_tone_wav as media_write_placeholder_tone_wav,
};
use muse_music::{Score, compose as compose_score_inner};
use muse_parser::parse;
use muse_project::{
    find_entry, hash_file, import_asset, init as project_init, list_skills, load_manifest,
    media_info, validate_manifest,
};
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
pub fn compile_source(
    source: String,
    settings: Option<RenderSettingsInput>,
    asset_root: Option<String>,
) -> Result<CompileResult> {
    let compilation: Compilation = compile_with(
        &source,
        settings_from_input(settings),
        CompileOptions {
            asset_root: asset_root.map(PathBuf::from),
        },
    );
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

/// Compose a deterministic score JSON from Muse source (music domain).
#[napi]
pub fn compose_score(source: String) -> Result<Value> {
    let score = compose_score_inner(&source).map_err(to_napi_err)?;
    json_value(&score)
}

fn score_from_value(score: Value) -> Result<Score> {
    serde_json::from_value(score).map_err(to_napi_err)
}

#[napi]
pub fn write_score_wav(score: Value, path: String) -> Result<()> {
    muse_music::write_wav(&score_from_value(score)?, PathBuf::from(path).as_path()).map_err(to_napi_err)
}

#[napi]
pub fn write_score_mp3(score: Value, path: String) -> Result<()> {
    muse_music::write_mp3(&score_from_value(score)?, PathBuf::from(path).as_path()).map_err(to_napi_err)
}

#[napi]
pub fn write_score_midi(score: Value, path: String) -> Result<()> {
    muse_music::write_midi(&score_from_value(score)?, PathBuf::from(path).as_path()).map_err(to_napi_err)
}

#[napi]
pub fn write_score_musicxml(score: Value, path: String) -> Result<()> {
    muse_music::write_musicxml(&score_from_value(score)?, PathBuf::from(path).as_path())
        .map_err(to_napi_err)
}

#[napi]
pub fn write_score_pdf(score: Value, path: String) -> Result<()> {
    muse_score::write_pdf(&score_from_value(score)?, PathBuf::from(path).as_path()).map_err(to_napi_err)
}

#[napi]
pub fn write_score_svg(score: Value, path: String) -> Result<()> {
    muse_score::write_svg(&score_from_value(score)?, PathBuf::from(path).as_path()).map_err(to_napi_err)
}

#[napi]
pub fn write_score_html(score: Value, path: String) -> Result<()> {
    muse_score::write_html(&score_from_value(score)?, PathBuf::from(path).as_path()).map_err(to_napi_err)
}

/// Load MIDI/MusicXML into score JSON.
#[napi]
pub fn load_score_file(path: String) -> Result<Value> {
    let score = muse_music::load_score(PathBuf::from(path).as_path()).map_err(to_napi_err)?;
    json_value(&score)
}

#[napi(object)]
pub struct EncodeMp4Options {
    pub asset_root: Option<String>,
    pub audio_wav: Option<String>,
    pub max_duration_seconds: Option<f64>,
}

/// Encode a plan to MP4 via GPU/CPU compose + ffmpeg (encode only).
#[napi]
pub fn encode_plan_mp4(plan: Value, out_path: String, options: Option<EncodeMp4Options>) -> Result<()> {
    let plan: RenderPlan = serde_json::from_value(plan).map_err(to_napi_err)?;
    let opts = options.unwrap_or(EncodeMp4Options {
        asset_root: None,
        audio_wav: None,
        max_duration_seconds: None,
    });
    let mp4 = Mp4Options {
        asset_root: PathBuf::from(opts.asset_root.unwrap_or_else(|| ".".into())),
        audio_wav: opts.audio_wav.map(PathBuf::from),
        max_duration_seconds: opts.max_duration_seconds,
    };
    render_mp4_with(&plan, PathBuf::from(out_path).as_path(), mp4).map_err(to_napi_err)
}

#[napi]
pub fn hash_file_sha256(path: String) -> Result<Value> {
    json_value(&hash_file(PathBuf::from(path).as_path()).map_err(to_napi_err)?)
}

#[napi]
pub fn import_project_asset(project_path: String, input_path: String) -> Result<Value> {
    json_value(
        &import_asset(
            PathBuf::from(project_path).as_path(),
            PathBuf::from(input_path).as_path(),
        )
        .map_err(to_napi_err)?,
    )
}

#[napi]
pub fn read_media_info(path: String) -> Result<Value> {
    json_value(&media_info(PathBuf::from(path).as_path()).map_err(to_napi_err)?)
}

#[napi]
pub fn list_project_skills(project_path: String) -> Result<Value> {
    json_value(&list_skills(PathBuf::from(project_path).as_path()).map_err(to_napi_err)?)
}

/// Block on Muse MCP over stdio until the client disconnects (host transport primitive).
#[napi]
pub fn serve_mcp_stdio(project_path: String) -> Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(to_napi_err)?;
    runtime
        .block_on(muse_mcp::serve(PathBuf::from(project_path)))
        .map_err(to_napi_err)
}
