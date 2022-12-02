use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::utils::find_config_dir;

use super::secret::{self, Secret};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Failed to find home directory")]
    HomeDirNotFound,
    #[error("Failed read current directory")]
    CurrentDirNotFound,
    #[error("Failed to create directory: {path}")]
    DirCreationFailed { path: String },
    #[error("Failed to read secret's file: {path}")]
    SecretFileReadError { path: String },
    #[error("Seceret file failed to deserialize")]
    DeserializationError,
    #[error("Seceret file failed to deserialize")]
    IdCreationFailed,
}

#[derive(Debug, clap::Args)]
pub struct Args {
    /// Use global config
    #[clap(long)]
    pub global: bool,

    /// root config folder
    #[clap(long)]
    config_dir: Option<PathBuf>,
}

impl Args {
    pub fn config_dir(&self) -> Result<PathBuf, Error> {
        let config_dir = if let Some(config_dir) = self.config_dir.as_ref() {
            config_dir.clone()
        } else if self.global {
            dirs::home_dir()
                .ok_or(Error::HomeDirNotFound)?
                .join(".soroban")
        } else {
            let pwd = std::env::current_dir().map_err(|_| Error::CurrentDirNotFound)?;
            find_config_dir(pwd.clone()).unwrap_or_else(|_| pwd.join(".soroban"))
        };
        ensure_directory(config_dir)
    }

    pub fn identity_dir(&self) -> Result<PathBuf, Error> {
        ensure_directory(self.config_dir()?).map(|p| p.join("identities"))
    }

    pub fn identity_path(&self, name: &str) -> Result<PathBuf, Error> {
        self.identity_dir().map(|p| {
            let mut source = p.join(name);
            source.set_extension("toml");
            source
        })
    }

    pub fn read_idendity(&self, name: &str) -> Result<Secret, Error> {
        let path = self.identity_path(name)?;
        let data = fs::read(&path).map_err(|_| Error::SecretFileReadError {
            path: path.to_string_lossy().to_string(),
        })?;
        toml::from_slice::<Secret>(&data).map_err(|_| Error::DeserializationError)
    }

    pub fn write_identity(&self, name: &str, secret: &Secret) -> Result<(), Error> {
        let source = self.identity_path(name)?;
        let data = toml::to_string(secret).map_err(|_| Error::IdCreationFailed)?;
        println!("Writing to {}", source.display());
        std::fs::write(&source, &data).map_err(|_| Error::IdCreationFailed)
    }
}

fn ensure_directory(dir: PathBuf) -> Result<PathBuf, Error> {
    std::fs::create_dir_all(&dir).map_err(|_| dir_creation_failed(&dir))?;
    Ok(dir)
}

fn dir_creation_failed(p: &Path) -> Error {
    Error::DirCreationFailed {
        path: format!("{}", p.display()),
    }
}
