use crate::app::ScoreCommand;
use anyhow::Result;

pub(crate) fn run(command: ScoreCommand) -> Result<()> {
    match command {
        ScoreCommand::Info { file } => {
            let score = muse_music::load_score(&file)?;
            println!("{}", serde_json::to_string_pretty(&score)?);
        }
    }
    Ok(())
}
