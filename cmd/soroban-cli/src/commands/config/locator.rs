use clap::arg;
use serde::de::DeserializeOwned;
use std::{
    ffi::OsStr,
    fmt::Display,
    fs, io,
    path::{Path, PathBuf},
    str::FromStr,
};

use crate::{utils::find_config_dir, Pwd};

use super::{network::Network, secret::Secret};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Failed to find home directory")]
    HomeDirNotFound,
    #[error("Failed read current directory")]
    CurrentDirNotFound,
    #[error("Failed read current directory and no SOROBAN_CONFIG_HOME is set")]
    NoConfigEnvVar,
    #[error("Failed to create directory: {path:?}")]
    DirCreationFailed { path: PathBuf },
    #[error(
        "Failed to read secret's file: {path}.\nProbably need to use `soroban config identity add`"
    )]
    SecretFileRead { path: PathBuf },
    #[error(
        "Failed to read network file: {path};\nProbably need to use `soroban config network add`"
    )]
    NetworkFileRead { path: PathBuf },
    #[error(transparent)]
    Toml(#[from] toml::de::Error),
    #[error("Seceret file failed to deserialize")]
    Deserialization,
    #[error("Failed to write identity file:{filepath}: {error}")]
    IdCreationFailed { filepath: PathBuf, error: io::Error },
    #[error("Seceret file failed to deserialize")]
    NetworkDeserialization,
    #[error("Failed to write network file: {0}")]
    NetworkCreationFailed(std::io::Error),
    #[error("Error Identity directory is invalid: {name}")]
    IdentityList { name: String },
    // #[error("Config file failed to deserialize")]
    // CannotReadConfigFile,
    #[error("Config file failed to serialize")]
    ConfigSerialization,
    // #[error("Config file failed write")]
    // CannotWriteConfigFile,
    #[error("XDG_CONFIG_HOME env variable is not a valid path. Got {0}")]
    XdgConfigHome(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("Failed to remove {0}: {1}")]
    ConfigRemoval(String, String),
    #[error("Failed to find config {0} for {1}")]
    ConfigMissing(String, String),
    #[error(transparent)]
    String(#[from] std::string::FromUtf8Error),
}

#[derive(Debug, clap::Args, Default, Clone)]
#[group(skip)]
pub struct Args {
    /// Use global config
    #[arg(long)]
    pub global: bool,

    #[arg(long, help_heading = "TESTING_OPTIONS")]
    pub pwd: Option<PathBuf>,
}

impl Args {
    pub fn config_dir(&self) -> Result<PathBuf, Error> {
        if self.global {
            global_config_path()
        } else {
            self.local_config()
        }
    }

    pub fn local_config(&self) -> Result<PathBuf, Error> {
        let pwd = self.current_dir()?;
        Ok(find_config_dir(pwd.clone()).unwrap_or_else(|_| pwd.join(".soroban")))
    }

    pub fn current_dir(&self) -> Result<PathBuf, Error> {
        self.pwd.as_ref().map_or_else(
            || std::env::current_dir().map_err(|_| Error::CurrentDirNotFound),
            |pwd| Ok(pwd.clone()),
        )
    }

    pub fn write_identity(&self, name: &str, secret: &Secret) -> Result<(), Error> {
        KeyType::Identity.write(name, secret, &self.config_dir()?)
    }

    pub fn write_network(&self, name: &str, network: &Network) -> Result<(), Error> {
        KeyType::Network.write(name, network, &self.config_dir()?)
    }

    pub fn list_identities(&self) -> Result<Vec<String>, Error> {
        let dir = self.config_dir()?;
        KeyType::Identity.list(&dir)
    }

    pub fn list_networks(&self) -> Result<Vec<String>, Error> {
        KeyType::Network.list(&self.config_dir()?)
    }
    pub fn read_identity(&self, name: &str) -> Result<Secret, Error> {
        KeyType::Identity.read_with_global(name, &self.local_config()?)
    }

    pub fn read_network(&self, name: &str) -> Result<Network, Error> {
        KeyType::Network.read_with_global(name, &self.local_config()?)
    }

    pub fn remove_identity(&self, name: &str) -> Result<(), Error> {
        KeyType::Identity.remove(name, &self.config_dir()?)
    }

    pub fn remove_network(&self, name: &str) -> Result<(), Error> {
        KeyType::Network.remove(name, &self.config_dir()?)
    }
}

fn ensure_directory(dir: PathBuf) -> Result<PathBuf, Error> {
    let parent = dir.parent().ok_or(Error::HomeDirNotFound)?;
    std::fs::create_dir_all(parent).map_err(|_| dir_creation_failed(parent))?;
    Ok(dir)
}

fn dir_creation_failed(p: &Path) -> Error {
    Error::DirCreationFailed {
        path: p.to_path_buf(),
    }
}

fn read_dir(dir: &Path) -> Result<Vec<String>, Error> {
    let contents = std::fs::read_dir(dir)?;
    let mut res = vec![];
    for entry in contents.filter_map(Result::ok) {
        let path = entry.path();
        if let Some("toml") = path.extension().and_then(OsStr::to_str) {
            if let Some(os_str) = path.file_stem() {
                res.push(os_str.to_string_lossy().trim().to_string());
            }
        }
    }
    res.sort();
    Ok(res)
}

pub enum KeyType {
    Identity,
    Network,
}

impl Display for KeyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                KeyType::Identity => "identity",
                KeyType::Network => "network",
            }
        )
    }
}

impl KeyType {
    pub fn read<T: DeserializeOwned>(&self, key: &str, pwd: &Path) -> Result<T, Error> {
        let path = self.path(pwd, key);
        let data = fs::read(&path).map_err(|_| Error::NetworkFileRead { path })?;
        let res = toml::from_slice(data.as_slice());
        Ok(res?)
    }

    pub fn read_with_global<T: DeserializeOwned>(&self, key: &str, pwd: &Path) -> Result<T, Error> {
        for path in [pwd, global_config_path()?.as_path()] {
            match self.read(key, path) {
                Ok(t) => return Ok(t),
                _ => continue,
            }
        }
        Err(Error::ConfigMissing(self.to_string(), key.to_string()))
    }

    pub fn write<T: serde::Serialize>(
        &self,
        key: &str,
        value: &T,
        pwd: &Path,
    ) -> Result<(), Error> {
        let filepath = ensure_directory(self.path(pwd, key))?;
        let data = toml::to_string(value).map_err(|_| Error::ConfigSerialization)?;
        std::fs::write(&filepath, data).map_err(|error| Error::IdCreationFailed { filepath, error })
    }

    fn root(&self, pwd: &Path) -> PathBuf {
        pwd.join(self.to_string())
    }

    fn path(&self, pwd: &Path, key: &str) -> PathBuf {
        let mut path = self.root(pwd).join(key);
        path.set_extension("toml");
        path
    }

    pub fn list(&self, pwd: &Path) -> Result<Vec<String>, Error> {
        let path = self.root(pwd);
        if path.exists() {
            Ok(read_dir(&path)?)
        } else {
            Ok(vec![])
        }
    }

    pub fn remove(&self, key: &str, pwd: &Path) -> Result<(), Error> {
        let path = self.path(pwd, key);
        if path.exists() {
            std::fs::remove_file(&path)
                .map_err(|_| Error::ConfigRemoval(self.to_string(), key.to_string()))
        } else {
            Ok(())
        }
    }
}

fn global_config_path() -> Result<PathBuf, Error> {
    Ok(if let Ok(config_home) = std::env::var("XDG_CONFIG_HOME") {
        PathBuf::from_str(&config_home).map_err(|_| Error::XdgConfigHome(config_home))?
    } else {
        dirs::home_dir()
            .ok_or(Error::HomeDirNotFound)?
            .join(".config")
    }
    .join("soroban"))
}

impl Pwd for Args {
    fn set_pwd(&mut self, pwd: &Path) {
        self.pwd = Some(pwd.to_path_buf());
    }
}
