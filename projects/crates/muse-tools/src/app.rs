use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "muse", version, about = "Muse audiovisual composition tools")]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Create a Muse project with a starter script.
    Init { path: Option<PathBuf> },
    /// Parse and validate a Muse source file or project (`--project`).
    Check(CheckArgs),
    /// Compile a Muse source file and print its render plan as JSON.
    Compile(CompileArgs),
    /// Render a low-resolution PNG/WAV preview (file or `--project --entry`).
    Preview(PreviewArgs),
    /// Render a score or audiovisual output.
    Render(RenderArgs),
    /// Export a composed score to a portable format.
    Export(ExportArgs),
    /// Import and inspect local assets.
    Asset {
        #[command(subcommand)]
        command: AssetCommand,
    },
    /// Inspect MIDI and MusicXML scores.
    Score {
        #[command(subcommand)]
        command: ScoreCommand,
    },
    /// Inspect project-local Muse skill manifests.
    Skill {
        #[command(subcommand)]
        command: SkillCommand,
    },
    /// Start the Muse Model Context Protocol server over stdio.
    Mcp {
        #[command(subcommand)]
        command: McpCommand,
    },
}

#[derive(Debug, Args)]
pub(crate) struct CheckArgs {
    /// Script to check when not using `--project`.
    pub(crate) script: Option<PathBuf>,
    /// Project root containing `muse.von`.
    #[arg(long)]
    pub(crate) project: Option<PathBuf>,
    /// Entry id when checking a multi-entry project.
    #[arg(long)]
    pub(crate) entry: Option<String>,
}

#[derive(Debug, Args)]
pub(crate) struct PreviewArgs {
    /// Script to preview when not using `--project`.
    pub(crate) script: Option<PathBuf>,
    /// Project root containing `muse.von`.
    #[arg(long)]
    pub(crate) project: Option<PathBuf>,
    /// Entry id when previewing a project.
    #[arg(long)]
    pub(crate) entry: Option<String>,
    /// Linear scale applied to plan width/height (0.01–1.0). Default 0.1.
    #[arg(long, default_value_t = 0.1)]
    pub(crate) scale: f32,
    /// Hard cap on preview frames. Default 240.
    #[arg(long, default_value_t = 240)]
    pub(crate) max_frames: usize,
    /// Hard cap on guide WAV seconds. Default 10.
    #[arg(long, default_value_t = 10.0)]
    pub(crate) max_audio_seconds: f64,
}

#[derive(Debug, Args)]
pub(crate) struct CompileArgs {
    pub(crate) script: PathBuf,
    /// Render plan emission format.
    #[arg(long, value_enum, default_value_t = EmitFormat::Json)]
    pub(crate) emit: EmitFormat,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum EmitFormat {
    Json,
    Schema,
}

#[derive(Debug, Args)]
pub(crate) struct RenderArgs {
    /// Script to render when not using `--project`.
    pub(crate) script: Option<PathBuf>,
    /// Project root containing `muse.von`.
    #[arg(long)]
    pub(crate) project: Option<PathBuf>,
    /// Entry id when rendering a project.
    #[arg(long)]
    pub(crate) entry: Option<String>,
    /// Output id from `muse.von` when using `--project` (defaults to first video/audio matching format).
    #[arg(long)]
    pub(crate) output_id: Option<String>,
    /// Target file to write. Optional when `--project` declares a matching output path.
    #[arg(short, long)]
    pub(crate) output: Option<PathBuf>,
    /// Output format. Defaults to the output path extension.
    #[arg(long, value_enum)]
    pub(crate) format: Option<RenderFormat>,
    /// Optional hard cap on MP4 duration seconds (smoke / partial renders).
    #[arg(long)]
    pub(crate) max_duration: Option<f64>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum RenderFormat {
    Wav,
    Mp3,
    Mp4,
}

#[derive(Debug, Args)]
pub(crate) struct ExportArgs {
    pub(crate) script: PathBuf,
    #[arg(short, long)]
    pub(crate) output: PathBuf,
    #[arg(long, value_enum)]
    pub(crate) format: ExportFormat,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum ExportFormat {
    Midi,
    Musicxml,
    Pdf,
    Svg,
    Html,
}

#[derive(Debug, Subcommand)]
pub(crate) enum AssetCommand {
    /// Copy a local file into the project asset directory and create metadata.
    Import { file: PathBuf },
    /// Print detected metadata for an asset file.
    Info { file: PathBuf },
}

#[derive(Debug, Subcommand)]
pub(crate) enum ScoreCommand {
    /// Print normalized score information for a MIDI or MusicXML file.
    Info { file: PathBuf },
}

#[derive(Debug, Subcommand)]
pub(crate) enum SkillCommand {
    /// List local project skill manifests.
    List,
    /// Validate local project skill manifests.
    Validate,
}

#[derive(Debug, Subcommand)]
pub(crate) enum McpCommand {
    /// Serve MCP requests over standard input and output.
    Serve {
        /// Muse project directory. Defaults to the current directory.
        #[arg(long, default_value = ".")]
        project: PathBuf,
    },
}
