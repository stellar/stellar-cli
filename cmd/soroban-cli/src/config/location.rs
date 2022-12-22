use std::{
    ffi::OsStr,
    fs, io,
    path::{Path, PathBuf},
};

use crate::utils::find_config_dir;

use super::{network::Network, secret::Secret, Config};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Failed to find home directory")]
    HomeDirNotFound,
    #[error("Failed read current directory")]
    CurrentDirNotFound,
    #[error("Failed to create directory: {path:?}")]
    DirCreationFailed { path: PathBuf },
    #[error("Failed to read secret's file: {path}")]
    SecretFileRead { path: String },
    #[error("Failed to read network file: {path}")]
    NetworkFileRead { path: String },
    #[error("Seceret file failed to deserialize")]
    Deserialization,
    #[error("Failed to write identity file:{filepath}: {error}")]
    IdCreationFailed {
        filepath: std::path::PathBuf,
        error: io::Error,
    },
    #[error("Seceret file failed to deserialize")]
    NetworkDeserialization,
    #[error("Failed to write network file")]
    NetworkCreationFailed,
    #[error("Error Identity directory is invalid: {name}")]
    IdentityList { name: String },
    #[error("Config file failed to deserialize")]
    CannotReadConfigFile,
    #[error("Config file failed to serialize")]
    ConfigSerialization,
    #[error("Config file failed write")]
    CannotWriteConfigFile,
}

#[derive(Debug, clap::Args, Default)]
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
        ensure_directory(self.config_dir()?.join("identities"))
    }

    pub fn network_dir(&self) -> Result<PathBuf, Error> {
        ensure_directory(self.config_dir()?.join("networks"))
    }

    pub fn identity_path(&self, name: &str) -> Result<PathBuf, Error> {
        self.identity_dir().map(|p| {
            let mut source = p.join(name);
            source.set_extension("toml");
            source
        })
    }

    pub fn network_path(&self, name: &str) -> Result<PathBuf, Error> {
        self.identity_dir().map(|p| {
            let mut source = p.join(name);
            source.set_extension("toml");
            source
        })
    }

    #[allow(dead_code)]
    pub fn read_identity(&self, name: &str) -> Result<Secret, Error> {
        let path = self.identity_path(name)?;
        let data = fs::read(&path).map_err(|_| Error::SecretFileRead {
            path: path.to_string_lossy().to_string(),
        })?;
        toml::from_slice::<Secret>(&data).map_err(|_| Error::Deserialization)
    }

    #[allow(dead_code)]
    pub fn read_network(&self, name: &str) -> Result<Network, Error> {
        let path = self.network_path(name)?;
        let data = fs::read(&path).map_err(|_| Error::NetworkFileRead {
            path: path.to_string_lossy().to_string(),
        })?;
        toml::from_slice::<Network>(&data).map_err(|_| Error::NetworkDeserialization)
    }

    pub fn write_identity(&self, name: &str, secret: &Secret) -> Result<(), Error> {
        let source = self.identity_path(name)?;
        let data = toml::to_string(secret).map_err(|_| Error::ConfigSerialization)?;
        std::fs::write(&source, data).map_err(|error| Error::IdCreationFailed {
            filepath: source.clone(),
            error,
        })
    }

    pub fn write_network(&self, name: &str, network: &Network) -> Result<(), Error> {
        let source = self.identity_path(name)?;
        let data = toml::to_string(network).map_err(|_| Error::Deserialization)?;
        std::fs::write(source, data).map_err(|_| Error::NetworkCreationFailed)
    }

    pub fn list_identities(&self) -> Result<Vec<String>, Error> {
        let path = self.identity_dir()?;
        read_dir(&path)
    }

    pub fn list_networks(&self) -> Result<Vec<String>, Error> {
        let path = self.network_dir()?;
        read_dir(&path)
    }

    pub fn config_path(&self) -> Result<PathBuf, Error> {
        Ok(self.config_dir()?.join("config.toml"))
    }

    pub fn get_config_file(&self) -> Result<Config, Error> {
        let path = self.config_path()?;
        if path.exists() {
            let data = fs::read(&path).map_err(|_| Error::CannotReadConfigFile)?;
            toml::from_slice::<Config>(&data).map_err(|_| Error::Deserialization)
        } else {
            Ok(Config::default())
        }
    }

    pub fn write_config_file(&self, config: &Config) -> Result<(), Error> {
        let path = self.config_path()?;
        let data = toml::to_string(config).map_err(|_| Error::ConfigSerialization)?;
        fs::write(path, data).map_err(|_| Error::CannotWriteConfigFile)
    }

    pub fn set_default_identity(&self, identity: &str) -> Result<(), Error> {
        let mut config = self.get_config_file()?;
        config.default_identity = Some(identity.to_owned());
        self.write_config_file(&config)
    }

    pub fn set_default_network(&self, network: &str) -> Result<(), Error> {
        let mut config = self.get_config_file()?;
        config.default_network = Some(network.to_owned());
        self.write_config_file(&config)
    }
}

fn ensure_directory(dir: PathBuf) -> Result<PathBuf, Error> {
    dbg!("creating directory {:?}", &dir);
    std::fs::create_dir_all(&dir).map_err(|_| dir_creation_failed(&dir))?;
    Ok(dir)
}

fn dir_creation_failed(p: &Path) -> Error {
    Error::DirCreationFailed {
        path: p.to_path_buf(),
    }
}

fn read_dir(dir: &Path) -> Result<Vec<String>, Error> {
    let contents = std::fs::read_dir(dir).map_err(|_| Error::IdentityList {
        name: format!("{}", dir.display()),
    })?;
    let mut res = vec![];
    for entry in contents.filter_map(Result::ok) {
        let path = entry.path();
        if let Some("toml") = path.extension().and_then(OsStr::to_str) {
            if let Some(os_str) = path.file_stem() {
                res.push(format!("{}", os_str.to_string_lossy()));
            }
        }
    }
    Ok(res)
}
