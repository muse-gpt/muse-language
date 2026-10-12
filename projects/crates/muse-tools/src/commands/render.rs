use crate::{
    app::{ExportArgs, ExportFormat, PreviewArgs, RenderArgs, RenderFormat},
    commands::check_compile,
};
use anyhow::{Context, Result, bail};
use std::{
    env, fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

fn ensure_parent(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        fs::create_dir_all(parent)
            .with_context(|| format!("cannot create output directory {}", parent.display()))?;
    }
    Ok(())
}

pub(crate) fn preview(args: &PreviewArgs) -> Result<()> {
    let (script, output, asset_root) = resolve_preview_paths(args)?;
    let plan = compile_with_project_settings(&script, args.project.as_deref())?;
    let manifest = muse_media::render_preview_with(
        &plan,
        &output,
        muse_media::PreviewOptions {
            scale: args.scale,
            max_frames: args.max_frames,
            max_audio_seconds: args.max_audio_seconds,
            asset_root,
        },
    )?;
    println!(
        "preview written to {} ({} frames, {:.2}s rendered / {:.2}s plan, audio {:.2}s)",
        output.display(),
        manifest.frames,
        manifest.rendered_duration_seconds,
        manifest.plan_duration_seconds,
        manifest.audio_seconds
    );
    Ok(())
}

fn resolve_preview_paths(args: &PreviewArgs) -> Result<(PathBuf, PathBuf, PathBuf)> {
    if let Some(project) = args.project.as_deref() {
        let manifest = muse_project::load_manifest(project)?;
        let entry = muse_project::find_entry(&manifest, args.entry.as_deref())?;
        let script = project.join(&entry.source);
        let output = entry
            .outputs
            .iter()
            .find(|output| matches!(output.kind, muse_project::OutputKind::Preview))
            .map(|output| project.join(&output.path))
            .unwrap_or_else(|| project.join("library").join(&entry.id).join("preview"));
        return Ok((script, output, project.to_path_buf()));
    }
    let script = args
        .script
        .clone()
        .context("pass a script path or --project <dir>")?;
    let output = script.parent().unwrap_or_else(|| Path::new(".")).join("preview");
    Ok((script.clone(), output, infer_asset_root(&script)))
}

fn infer_asset_root(script: &Path) -> PathBuf {
    for ancestor in script.ancestors() {
        if ancestor.join("muse.von").is_file() || ancestor.join("muse.toml").is_file() {
            return ancestor.to_path_buf();
        }
    }
    env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

fn compile_with_project_settings(script: &Path, project: Option<&Path>) -> Result<muse_ir::RenderPlan> {
    let source = check_compile::source_file(script)?;
    let settings = if let Some(project) = project {
        let manifest = muse_project::load_manifest(project)?;
        muse_types::RenderSettings {
            width: manifest.render.width,
            height: manifest.render.height,
            frame_rate: manifest.render.frame_rate,
            sample_rate: muse_types::RenderSettings::default().sample_rate,
        }
    } else {
        muse_types::RenderSettings::default()
    };
    muse_ir::compile(&source, settings)
        .plan
        .context("cannot use an invalid script")
}

pub(crate) fn render(args: &RenderArgs) -> Result<()> {
    let (script, output, asset_root, format) = resolve_render_target(args)?;
    ensure_parent(&output)?;
    let source = check_compile::source_file(&script)?;
    match format {
        RenderFormat::Mp4 => {
            let plan = compile_with_project_settings(&script, args.project.as_deref())?;
            let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
            let wav = env::temp_dir().join(format!("muse-render-audio-{stamp}.wav"));
            let score = muse_music::compose(&source)
                .with_context(|| format!("cannot compose {}", script.display()))?;
            muse_music::write_wav(&score, &wav)?;
            muse_media::render_mp4_with(
                &plan,
                &output,
                muse_media::Mp4Options {
                    asset_root,
                    audio_wav: Some(wav.clone()),
                    max_duration_seconds: args.max_duration,
                },
            )?;
            let _ = fs::remove_file(wav);
        }
        RenderFormat::Wav => {
            let score =
                muse_music::compose(&source).with_context(|| format!("cannot compose {}", script.display()))?;
            muse_music::write_wav(&score, &output)?;
        }
        RenderFormat::Mp3 => {
            let score =
                muse_music::compose(&source).with_context(|| format!("cannot compose {}", script.display()))?;
            muse_music::write_mp3(&score, &output)?;
        }
    }
    println!("rendered {}", output.display());
    Ok(())
}

fn resolve_render_target(args: &RenderArgs) -> Result<(PathBuf, PathBuf, PathBuf, RenderFormat)> {
    if let Some(project) = args.project.as_deref() {
        let manifest = muse_project::load_manifest(project)?;
        let entry = muse_project::find_entry(&manifest, args.entry.as_deref())?;
        let script = project.join(&entry.source);
        let format = args
            .format
            .or_else(|| args.output.as_ref().and_then(|path| format_from_path(path).ok()))
            .unwrap_or(RenderFormat::Mp4);
        let output = if let Some(path) = &args.output {
            path.clone()
        } else if let Some(id) = &args.output_id {
            let spec = entry
                .outputs
                .iter()
                .find(|output| output.id == *id)
                .with_context(|| format!("unknown output id `{id}` on entry `{}`", entry.id))?;
            project.join(&spec.path)
        } else {
            let kind = match format {
                RenderFormat::Mp4 => muse_project::OutputKind::Video,
                RenderFormat::Wav | RenderFormat::Mp3 => muse_project::OutputKind::Audio,
            };
            entry
                .outputs
                .iter()
                .find(|output| output.kind == kind)
                .map(|output| project.join(&output.path))
                .unwrap_or_else(|| {
                    let ext = match format {
                        RenderFormat::Mp4 => "mp4",
                        RenderFormat::Wav => "wav",
                        RenderFormat::Mp3 => "mp3",
                    };
                    project
                        .join("library")
                        .join(&entry.id)
                        .join(format!("out.{ext}"))
                })
        };
        return Ok((script, output, project.to_path_buf(), format));
    }
    let script = args
        .script
        .clone()
        .context("pass a script path or --project <dir>")?;
    let output = args
        .output
        .clone()
        .context("pass --output <path> when not using --project")?;
    let format = match args.format {
        Some(format) => format,
        None => format_from_path(&output)?,
    };
    Ok((script.clone(), output, infer_asset_root(&script), format))
}

pub(crate) fn export(args: &ExportArgs) -> Result<()> {
    ensure_parent(&args.output)?;
    let source = check_compile::source_file(&args.script)?;
    let score = muse_music::compose(&source).with_context(|| format!("cannot compose {}", args.script.display()))?;
    match args.format {
        ExportFormat::Midi => muse_music::write_midi(&score, &args.output)?,
        ExportFormat::Musicxml => muse_music::write_musicxml(&score, &args.output)?,
        ExportFormat::Pdf => muse_score::write_pdf(&score, &args.output)?,
        ExportFormat::Svg => muse_score::write_svg(&score, &args.output)?,
        ExportFormat::Html => muse_score::write_html(&score, &args.output)?,
    }
    println!("exported {}", args.output.display());
    Ok(())
}

fn format_from_path(path: &Path) -> Result<RenderFormat> {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("wav") => Ok(RenderFormat::Wav),
        Some("mp3") => Ok(RenderFormat::Mp3),
        Some("mp4") | None => Ok(RenderFormat::Mp4),
        Some(extension) => bail!("unsupported render output extension `.{extension}`; use .wav, .mp3, or .mp4"),
    }
}
