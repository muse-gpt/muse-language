use crate::{
    app::{ExportArgs, ExportFormat, PreviewArgs, RenderArgs, RenderFormat},
    commands::check_compile,
};
use anyhow::{Context, Result, bail};
use std::{fs, path::Path};

fn ensure_parent(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        fs::create_dir_all(parent)
            .with_context(|| format!("cannot create output directory {}", parent.display()))?;
    }
    Ok(())
}

pub(crate) fn preview(args: &PreviewArgs) -> Result<()> {
    let (script, output) = resolve_preview_paths(args)?;
    let plan = check_compile::compile_file(&script)?
        .plan
        .context("cannot preview an invalid script")?;
    let settings = muse_types::RenderSettings {
        width: plan.settings.width,
        height: plan.settings.height,
        frame_rate: plan.settings.frame_rate,
        sample_rate: plan.settings.sample_rate,
    };
    // Recompile with project render defaults when available.
    let plan = if let Some(project) = args.project.as_deref() {
        let manifest = muse_project::load_manifest(project)?;
        let source = check_compile::source_file(&script)?;
        muse_ir::compile(
            &source,
            muse_types::RenderSettings {
                width: manifest.render.width,
                height: manifest.render.height,
                frame_rate: manifest.render.frame_rate,
                sample_rate: settings.sample_rate,
            },
        )
        .plan
        .context("cannot preview an invalid script")?
    } else {
        plan
    };
    let manifest = muse_media::render_preview_with(
        &plan,
        &output,
        muse_media::PreviewOptions {
            scale: args.scale,
            max_frames: args.max_frames,
            max_audio_seconds: args.max_audio_seconds,
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

fn resolve_preview_paths(args: &PreviewArgs) -> Result<(std::path::PathBuf, std::path::PathBuf)> {
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
        return Ok((script, output));
    }
    let script = args
        .script
        .clone()
        .context("pass a script path or --project <dir>")?;
    let output = script.parent().unwrap_or_else(|| Path::new(".")).join("preview");
    Ok((script, output))
}

pub(crate) fn render(args: &RenderArgs) -> Result<()> {
    let format = match args.format {
        Some(format) => format,
        None => format_from_path(&args.output)?,
    };
    ensure_parent(&args.output)?;
    let source = check_compile::source_file(&args.script)?;
    match format {
        RenderFormat::Mp4 => {
            let compilation = muse_ir::compile(&source, muse_types::RenderSettings::default());
            let plan = compilation.plan.context("cannot render an invalid script")?;
            muse_media::render_mp4(&plan, &args.output)?;
        }
        RenderFormat::Wav => {
            let score =
                muse_music::compose(&source).with_context(|| format!("cannot compose {}", args.script.display()))?;
            muse_music::write_wav(&score, &args.output)?;
        }
        RenderFormat::Mp3 => {
            let score =
                muse_music::compose(&source).with_context(|| format!("cannot compose {}", args.script.display()))?;
            muse_music::write_mp3(&score, &args.output)?;
        }
    }
    println!("rendered {}", args.output.display());
    Ok(())
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
