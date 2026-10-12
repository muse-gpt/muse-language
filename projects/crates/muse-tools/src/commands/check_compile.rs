use crate::app::{CheckArgs, CompileArgs};
use anyhow::{Context, Result, bail};
use muse_project::OutputKind;
use muse_types::RenderSettings;
use std::{fs, path::Path};

pub(crate) fn check(args: &CheckArgs) -> Result<()> {
    if let Some(project) = args.project.as_deref() {
        return check_project(project, args.entry.as_deref());
    }
    let script = args
        .script
        .as_deref()
        .context("pass a script path or --project <dir>")?;
    let compilation = compile_file(script)?;
    print_diagnostics(&compilation);
    if !compilation.is_success() {
        bail!("validation failed");
    }
    Ok(())
}

fn check_project(project: &Path, entry_id: Option<&str>) -> Result<()> {
    let manifest = muse_project::load_manifest(project)?;
    let entries: Vec<_> = match entry_id {
        Some(id) => vec![muse_project::find_entry(&manifest, Some(id))?],
        None => manifest.entries.iter().collect(),
    };
    let mut failed = false;
    for entry in entries {
        let source = project.join(&entry.source);
        println!("check entry `{}` ({})", entry.id, source.display());
        let compilation = compile_file(&source)?;
        print_diagnostics(&compilation);
        if !compilation.is_success() {
            failed = true;
            continue;
        }
        for output in &entry.outputs {
            if output.path.trim().is_empty() {
                bail!("entry `{}` output `{}` has an empty path", entry.id, output.id);
            }
            if matches!(output.kind, OutputKind::Preview | OutputKind::Video | OutputKind::Audio)
                && Path::new(&output.path).is_absolute()
            {
                bail!(
                    "entry `{}` output `{}` must use a project-relative path",
                    entry.id,
                    output.id
                );
            }
        }
    }
    if failed {
        bail!("project validation failed");
    }
    println!(
        "project ok ({})",
        muse_project::manifest_path(project)?.display()
    );
    Ok(())
}

pub(crate) fn compile(args: &CompileArgs) -> Result<()> {
    match args.emit {
        crate::app::EmitFormat::Schema => {
            println!("{}", serde_json::to_string_pretty(&muse_ir::compilation_schema())?);
            return Ok(());
        }
        crate::app::EmitFormat::Json => {}
    }
    let compilation = compile_file(&args.script)?;
    print_diagnostics(&compilation);
    let plan = compilation.plan.context("cannot emit a render plan with errors")?;
    println!("{}", serde_json::to_string_pretty(&plan)?);
    Ok(())
}

pub(crate) fn compile_file(path: &Path) -> Result<muse_ir::Compilation> {
    let source = fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
    Ok(muse_ir::compile(&source, RenderSettings::default()))
}

pub(crate) fn source_file(path: &Path) -> Result<String> {
    fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))
}

pub(crate) fn print_diagnostics(compilation: &muse_ir::Compilation) {
    for diagnostic in &compilation.diagnostics {
        eprintln!(
            "{:?}:{}:{}: {}",
            diagnostic.severity, diagnostic.span.line, diagnostic.span.column, diagnostic.message
        );
    }
}
