use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::utils::find_config_dir;

use super::secret::Secret;

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
    #[error("Error Identity directory is invalid: {name}")]
    IdentityList { name: String },
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

    pub fn read_identity(&self, name: &str) -> Result<Secret, Error> {
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

    pub fn list_identities(&self) -> Result<Vec<String>, Error> {
        let path = self.identity_dir()?;
        let contents = std::fs::read_dir(&path).map_err(|_| Error::IdentityList {
            name: format!("{}", path.display()),
        })?;
        let mut res = vec![];
        for entry in contents.filter_map(Result::ok) {
            let path = entry.path();
            if let Some("toml") = path.extension().and_then(|s| s.to_str()) {
                if let Some(os_str) = path.file_stem() {
                    res.push(format!("{}", os_str.to_string_lossy()))
                }
            }
        }
        Ok(res)
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
