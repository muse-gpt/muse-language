use crate::app::AssetCommand;
use anyhow::Result;
use std::path::Path;

pub(crate) fn run(command: AssetCommand) -> Result<()> {
    match command {
        AssetCommand::Import { file } => {
            let meta = muse_project::import_asset(Path::new("."), &file)?;
            println!("imported {} ({})", meta.guid, meta.hash);
        }
        AssetCommand::Info { file } => {
            let info = muse_project::media_info(&file)?;
            println!("{}", serde_json::to_string_pretty(&info)?);
        }
    }
    Ok(())
}
