use crate::app::CompileArgs;
use anyhow::{Context, Result, bail};
use muse_types::RenderSettings;
use std::{fs, path::Path};

pub(crate) fn check(path: &Path) -> Result<()> {
    let compilation = compile_file(path)?;
    print_diagnostics(&compilation);
    if !compilation.is_success() {
        bail!("validation failed");
    }
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
