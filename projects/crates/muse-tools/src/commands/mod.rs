mod assets;
mod check_compile;
mod mcp;
mod project;
mod render;
mod score;

use crate::app::{Cli, Command};
use anyhow::Result;

pub(crate) async fn dispatch(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Init { path } => project::init(path.as_deref()),
        Command::Check { script } => check_compile::check(&script),
        Command::Compile(args) => check_compile::compile(&args),
        Command::Preview { script } => render::preview(&script),
        Command::Render(args) => render::render(&args),
        Command::Export(args) => render::export(&args),
        Command::Asset { command } => assets::run(command),
        Command::Score { command } => score::run(command),
        Command::Skill { command } => project::skill(command),
        Command::Mcp { command } => mcp::run(command).await,
    }
}
