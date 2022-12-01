use std::path::{PathBuf, Path};

use crate::utils::find_config_dir;


#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Failed to find home directory")]
    HomeDirNotFound,
    #[error("Failed read current directory")]
    CurrentDirNotFound,
    #[error("Failed to create directory: {path}")]
    DirCreationFailed { path: String },
}

#[derive(Debug, clap::Args)]
pub struct Args {
    /// Use global config
    #[clap(long)]
    pub global: bool,
}

impl Args {
    pub fn config_dir(&self) -> Result<PathBuf, Error> {
        if self.global {
            let dir = dirs::home_dir()
                .ok_or(Error::HomeDirNotFound)?
                .join(".soroban")
                .join("identities");
            if !dir.exists() {
                std::fs::create_dir_all(&dir).map_err(|_| dir_creation_failed(&dir))?;
            }
            Ok(dir)
        } else {
            let pwd = std::env::current_dir().map_err(|_| Error::CurrentDirNotFound)?;
            let config_location = find_config_dir(pwd.clone())
                .unwrap_or_else(|_| pwd.join(".soroban"))
                .join("identities");
            std::fs::create_dir_all(&config_location)
                .map_err(|_| dir_creation_failed(&config_location))?;
            Ok(config_location)
        }
    }
}

fn dir_creation_failed(p: &Path) -> Error {
    Error::DirCreationFailed {
        path: format!("{}", p.display()),
    }
}
