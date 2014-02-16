use crate::{
    app::{ExportArgs, ExportFormat, RenderArgs, RenderFormat},
    commands::check_compile,
};
use anyhow::{Context, Result, bail};
use std::path::Path;

pub(crate) fn preview(script: &Path) -> Result<()> {
    let plan = check_compile::compile_file(script)?
        .plan
        .context("cannot preview an invalid script")?;
    let output = script.parent().unwrap_or_else(|| Path::new(".")).join("preview");
    let manifest = muse_media::render_preview(&plan, &output, 0.1)?;
    println!("preview written to {} ({} frames)", output.display(), manifest.frames);
    Ok(())
}

pub(crate) fn render(args: &RenderArgs) -> Result<()> {
    let format = match args.format {
        Some(format) => format,
        None => format_from_path(&args.output)?,
    };
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
