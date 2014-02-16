mod app;
mod commands;

use anyhow::Result;
use clap::Parser;

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("muse: {error:#}");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    commands::dispatch(app::Cli::parse()).await
}
