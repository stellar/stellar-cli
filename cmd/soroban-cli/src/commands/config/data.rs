use chrono::prelude::*;
use directories::ProjectDirs;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Failed to find project directories")]
    FiledToFindProjectDirs,
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub fn project_dir() -> Result<directories::ProjectDirs, Error> {
    ProjectDirs::from("com", "stellar", "soroban-cli").ok_or(Error::FiledToFindProjectDirs)
}

pub fn write(prefix: &str, suffix: &str, data: impl AsRef<[u8]>) -> Result<(), Error> {
    let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
    let name = format!("{prefix}_{timestamp}{suffix}");
    let file = project_dir()?.data_local_dir().join(name);
    Ok(std::fs::write(file, data)?)
}
