use crate::app::SkillCommand;
use anyhow::{Result, bail};
use std::path::Path;

pub(crate) fn init(path: Option<&Path>) -> Result<()> {
    let path = path.unwrap_or_else(|| Path::new("."));
    muse_project::init(path)?;
    println!("initialized Muse project at {}", path.display());
    Ok(())
}

pub(crate) fn skill(command: SkillCommand) -> Result<()> {
    let skills = muse_project::list_skills(Path::new("."))?;
    match command {
        SkillCommand::List => {
            for skill in skills {
                println!("{} {} - {}", skill.name, skill.version, skill.description);
            }
        }
        SkillCommand::Validate => {
            for skill in skills {
                if skill.name.trim().is_empty() || skill.version.trim().is_empty() {
                    bail!("skill needs a name and version");
                }
                println!("valid: {} {}", skill.name, skill.version);
            }
        }
    }
    Ok(())
}
