use clap::arg;
use directories::UserDirs;
use itertools::Itertools;
use serde::de::DeserializeOwned;
use std::{
    ffi::OsStr,
    fmt::Display,
    fs::{self, create_dir_all, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    str::FromStr,
};
use stellar_strkey::{Contract, DecodeError};

use crate::{
    commands::{global, HEADING_GLOBAL},
    print::Print,
    signer::{self, keyring::StellarEntry},
    utils::find_config_dir,
    xdr, Pwd,
};

use super::{
    alias,
    key::{self, Key},
    network::{self, Network},
    secret::Secret,
    Config,
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    TomlSerialize(#[from] toml::ser::Error),
    #[error("Failed to find home directory")]
    HomeDirNotFound,
    #[error("Failed read current directory")]
    CurrentDirNotFound,
    #[error("Failed read current directory and no SOROBAN_CONFIG_HOME is set")]
    NoConfigEnvVar,
    #[error("Failed to create directory: {path:?}")]
    DirCreationFailed { path: PathBuf },
    #[error("Failed to read secret's file: {path}.\nProbably need to use `stellar keys add`")]
    SecretFileRead { path: PathBuf },
    #[error("Failed to read network file: {path};\nProbably need to use `stellar network add`")]
    NetworkFileRead { path: PathBuf },
    #[error("Failed to read file: {path}")]
    FileRead { path: PathBuf },
    #[error(transparent)]
    Toml(#[from] toml::de::Error),
    #[error("Secret file failed to deserialize")]
    Deserialization,
    #[error("Failed to write identity file:{filepath}: {error}")]
    IdCreationFailed { filepath: PathBuf, error: io::Error },
    #[error("Secret file failed to deserialize")]
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
    #[error(transparent)]
    Secret(#[from] crate::config::secret::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("cannot access config dir for alias file")]
    CannotAccessConfigDir,
    #[error("cannot access alias config file (no permission or doesn't exist)")]
    CannotAccessAliasConfigFile,
    #[error("cannot parse contract ID {0}: {1}")]
    CannotParseContractId(String, DecodeError),
    #[error("contract not found: {0}")]
    ContractNotFound(String),
    #[error("Failed to read upgrade check file: {path}: {error}")]
    UpgradeCheckReadFailed { path: PathBuf, error: io::Error },
    #[error("Failed to write upgrade check file: {path}: {error}")]
    UpgradeCheckWriteFailed { path: PathBuf, error: io::Error },
    #[error("Contract alias {0}, cannot overlap with key")]
    ContractAliasCannotOverlapWithKey(String),
    #[error("Key cannot {0} cannot overlap with contract alias")]
    KeyCannotOverlapWithContractAlias(String),
    #[error(transparent)]
    Keyring(#[from] signer::keyring::Error),
    #[error("Only private keys and seed phrases are supported for getting private keys {0}")]
    SecretKeyOnly(String),
    #[error(transparent)]
    Key(#[from] key::Error),
}

#[derive(Debug, clap::Args, Default, Clone)]
#[group(skip)]
#[cfg(feature = "version_lt_23")]
pub struct Args {
    /// Use global config
    #[arg(long, global = true, help_heading = HEADING_GLOBAL)]
    pub global: bool,

    /// Location of config directory, default is "."
    #[arg(long, global = true, help_heading = HEADING_GLOBAL)]
    pub config_dir: Option<PathBuf>,
}

#[derive(Debug, clap::Args, Default, Clone)]
#[group(skip)]
#[cfg(not(feature = "version_lt_23"))]
#[cfg(feature = "version_gte_23")]
pub struct Args {
    /// ⚠️ Deprecated: global config is always on
    #[arg(long, global = true, help_heading = HEADING_GLOBAL)]
    pub global: bool,

    /// Location of config directory, default is "$`XDG_CONFIG_HOME/.stellar`"
    #[arg(long, global = true, help_heading = HEADING_GLOBAL)]
    pub config_dir: Option<PathBuf>,
}

pub enum Location {
    Local(PathBuf),
    Global(PathBuf),
}

impl Display for Location {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} {:?}",
            match self {
                Location::Local(_) => "Local",
                Location::Global(_) => "Global",
            },
            self.as_ref()
        )
    }
}

impl AsRef<Path> for Location {
    fn as_ref(&self) -> &Path {
        match self {
            Location::Local(p) | Location::Global(p) => p.as_path(),
        }
    }
}

impl Location {
    #[must_use]
    pub fn wrap(&self, p: PathBuf) -> Self {
        match self {
            Location::Local(_) => Location::Local(p),
            Location::Global(_) => Location::Global(p),
        }
    }
}

impl Args {
    #[cfg(feature = "version_lt_23")]
    pub fn config_dir(&self) -> Result<PathBuf, Error> {
        if self.global {
            self.global_config_path()
        } else {
            self.local_config()
        }
    }

    #[cfg(not(feature = "version_lt_23"))]
    #[cfg(feature = "version_gte_23")]
    pub fn config_dir(&self) -> Result<PathBuf, Error> {
        if self.global {
            let print = Print::new(false);
            print.warnln("Flag --global is deprecated: global config is always used");
        }
        self.global_config_path()
    }

    pub fn local_and_global(&self) -> Result<[Location; 2], Error> {
        Ok([
            Location::Local(self.local_config()?),
            Location::Global(self.global_config_path()?),
        ])
    }

    pub fn local_config(&self) -> Result<PathBuf, Error> {
        let pwd = self.current_dir()?;
        Ok(find_config_dir(pwd.clone()).unwrap_or_else(|_| pwd.join(".stellar")))
    }

    pub fn current_dir(&self) -> Result<PathBuf, Error> {
        self.config_dir.as_ref().map_or_else(
            || std::env::current_dir().map_err(|_| Error::CurrentDirNotFound),
            |pwd| Ok(pwd.clone()),
        )
    }

    pub fn write_identity(&self, name: &str, secret: &Secret) -> Result<PathBuf, Error> {
        if let Ok(Some(_)) = self.load_contract_from_alias(name) {
            return Err(Error::KeyCannotOverlapWithContractAlias(name.to_owned()));
        }
        KeyType::Identity.write(name, secret, &self.config_dir()?)
    }

    pub fn write_public_key(
        &self,
        name: &str,
        public_key: &stellar_strkey::ed25519::PublicKey,
    ) -> Result<PathBuf, Error> {
        self.write_key(name, &public_key.into())
    }

    pub fn write_key(&self, name: &str, key: &Key) -> Result<PathBuf, Error> {
        KeyType::Identity.write(name, key, &self.config_dir()?)
    }

    pub fn write_network(&self, name: &str, network: &Network) -> Result<PathBuf, Error> {
        KeyType::Network.write(name, network, &self.config_dir()?)
    }

    pub fn write_default_network(&self, name: &str) -> Result<(), Error> {
        Config::new()?.set_network(name).save()
    }

    pub fn write_default_identity(&self, name: &str) -> Result<(), Error> {
        Config::new()?.set_identity(name).save()
    }

    pub fn list_identities(&self) -> Result<Vec<String>, Error> {
        Ok(KeyType::Identity
            .list_paths(&self.local_and_global()?)?
            .into_iter()
            .map(|(name, _)| name)
            .collect())
    }

    pub fn list_identities_long(&self) -> Result<Vec<(String, String)>, Error> {
        Ok(KeyType::Identity
            .list_paths(&self.local_and_global()?)
            .into_iter()
            .flatten()
            .map(|(name, location)| {
                let path = match location {
                    Location::Local(path) | Location::Global(path) => path,
                };
                (name, format!("{}", path.display()))
            })
            .collect())
    }

    pub fn list_networks(&self) -> Result<Vec<String>, Error> {
        let saved_networks = KeyType::Network
            .list_paths(&self.local_and_global()?)
            .into_iter()
            .flatten()
            .map(|x| x.0);
        let default_networks = network::DEFAULTS.keys().map(ToString::to_string);
        Ok(saved_networks.chain(default_networks).unique().collect())
    }

    pub fn list_networks_long(&self) -> Result<Vec<(String, Network, String)>, Error> {
        let saved_networks = KeyType::Network
            .list_paths(&self.local_and_global()?)
            .into_iter()
            .flatten()
            .filter_map(|(name, location)| {
                Some((
                    name,
                    KeyType::read_from_path::<Network>(location.as_ref()).ok()?,
                    location.to_string(),
                ))
            });
        let default_networks = network::DEFAULTS
            .into_iter()
            .map(|(name, network)| ((*name).to_string(), network.into(), "Default".to_owned()));
        Ok(saved_networks.chain(default_networks).collect())
    }

    pub fn read_identity(&self, name: &str) -> Result<Key, Error> {
        KeyType::Identity.read_with_global(name, self)
    }

    pub fn read_key(&self, key_or_name: &str) -> Result<Key, Error> {
        key_or_name
            .parse()
            .or_else(|_| self.read_identity(key_or_name))
    }

    pub fn get_secret_key(&self, key_or_name: &str) -> Result<Secret, Error> {
        match self.read_key(key_or_name)? {
            Key::Secret(s) => Ok(s),
            _ => Err(Error::SecretKeyOnly(key_or_name.to_string())),
        }
    }

    pub fn get_public_key(
        &self,
        key_or_name: &str,
        hd_path: Option<usize>,
    ) -> Result<xdr::MuxedAccount, Error> {
        Ok(self.read_key(key_or_name)?.muxed_account(hd_path)?)
    }

    pub fn read_network(&self, name: &str) -> Result<Network, Error> {
        let res = KeyType::Network.read_with_global(name, self);
        if let Err(Error::ConfigMissing(_, _)) = &res {
            let Some(network) = network::DEFAULTS.get(name) else {
                return res;
            };
            return Ok(network.into());
        }
        res
    }

    pub fn remove_identity(&self, name: &str, global_args: &global::Args) -> Result<(), Error> {
        let print = Print::new(global_args.quiet);
        let identity = self.read_identity(name)?;

        if let Key::Secret(Secret::SecureStore { entry_name }) = identity {
            let entry = StellarEntry::new(&entry_name)?;
            match entry.delete_seed_phrase() {
                Ok(()) => {}
                Err(e) => match e {
                    signer::keyring::Error::Keyring(keyring::Error::NoEntry) => {
                        print.infoln("This key was already removed from the secure store. Removing the cli config file.");
                    }
                    _ => {
                        return Err(Error::Keyring(e));
                    }
                },
            }
        }

        KeyType::Identity.remove(name, &self.config_dir()?)
    }

    pub fn remove_network(&self, name: &str) -> Result<(), Error> {
        KeyType::Network.remove(name, &self.config_dir()?)
    }

    #[cfg(feature = "version_lt_23")]
    fn load_contract_from_alias(&self, alias: &str) -> Result<Option<alias::Data>, Error> {
        let path = self.alias_path(alias)?;

        if !path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(path)?;
        let data: alias::Data = serde_json::from_str(&content).unwrap_or_default();

        Ok(Some(data))
    }

    #[cfg(not(feature = "version_lt_23"))]
    #[cfg(feature = "version_gte_23")]
    fn load_contract_from_alias(&self, alias: &str) -> Result<Option<alias::Data>, Error> {
        let file_name = format!("{alias}.json");
        let config_dirs = self.local_and_global()?;
        let local = &config_dirs[0];
        let global = &config_dirs[1];

        match local {
            Location::Local(config_dir) => {
                let path = config_dir.join("contract-ids").join(&file_name);
                if path.exists() {
                    print_deprecation_warning();

                    let content = fs::read_to_string(path)?;
                    let data: alias::Data = serde_json::from_str(&content).unwrap_or_default();

                    return Ok(Some(data));
                }
            }
            Location::Global(_) => unreachable!(),
        };

        match global {
            Location::Global(config_dir) => {
                let path = config_dir.join("contract-ids").join(&file_name);
                if !path.exists() {
                    return Ok(None);
                }

                let content = fs::read_to_string(path)?;
                let data: alias::Data = serde_json::from_str(&content).unwrap_or_default();

                Ok(Some(data))
            }
            Location::Local(_) => unreachable!(),
        }
    }

    fn alias_path(&self, alias: &str) -> Result<PathBuf, Error> {
        let file_name = format!("{alias}.json");
        let config_dir = self.config_dir()?;
        Ok(config_dir.join("contract-ids").join(file_name))
    }

    pub fn save_contract_id(
        &self,
        network_passphrase: &str,
        contract_id: &stellar_strkey::Contract,
        alias: &str,
    ) -> Result<(), Error> {
        if self.read_identity(alias).is_ok() {
            return Err(Error::ContractAliasCannotOverlapWithKey(alias.to_owned()));
        }
        let path = self.alias_path(alias)?;
        let dir = path.parent().ok_or(Error::CannotAccessConfigDir)?;

        create_dir_all(dir).map_err(|_| Error::CannotAccessConfigDir)?;

        let content = fs::read_to_string(&path).unwrap_or_default();
        let mut data: alias::Data = serde_json::from_str(&content).unwrap_or_default();

        let mut to_file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(path)?;

        data.ids
            .insert(network_passphrase.into(), contract_id.to_string());

        let content = serde_json::to_string(&data)?;

        Ok(to_file.write_all(content.as_bytes())?)
    }

    pub fn remove_contract_id(&self, network_passphrase: &str, alias: &str) -> Result<(), Error> {
        let path = self.alias_path(alias)?;

        if !path.is_file() {
            return Err(Error::CannotAccessAliasConfigFile);
        }

        let content = fs::read_to_string(&path).unwrap_or_default();
        let mut data: alias::Data = serde_json::from_str(&content).unwrap_or_default();

        let mut to_file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(path)?;

        data.ids.remove::<str>(network_passphrase);

        let content = serde_json::to_string(&data)?;

        Ok(to_file.write_all(content.as_bytes())?)
    }

    pub fn get_contract_id(
        &self,
        alias: &str,
        network_passphrase: &str,
    ) -> Result<Option<Contract>, Error> {
        let Some(alias_data) = self.load_contract_from_alias(alias)? else {
            return Ok(None);
        };

        alias_data
            .ids
            .get(network_passphrase)
            .map(|id| id.parse())
            .transpose()
            .map_err(|e| Error::CannotParseContractId(alias.to_owned(), e))
    }

    pub fn resolve_contract_id(
        &self,
        alias_or_contract_id: &str,
        network_passphrase: &str,
    ) -> Result<Contract, Error> {
        let Some(contract) = self.get_contract_id(alias_or_contract_id, network_passphrase)? else {
            return alias_or_contract_id
                .parse()
                .map_err(|e| Error::CannotParseContractId(alias_or_contract_id.to_owned(), e));
        };
        Ok(contract)
    }

    pub fn global_config_path(&self) -> Result<PathBuf, Error> {
        #[cfg(feature = "version_gte_23")]
        if let Some(config_dir) = &self.config_dir {
            return Ok(config_dir.clone());
        }

        global_config_path()
    }
}

#[cfg(feature = "version_gte_23")]
pub fn print_deprecation_warning() {
    let print = Print::new(false);
    print.warnln("Local config is deprecated and will be removed in the future");
    print.warnln("To resolve this warning run 'stellar config migrate'".to_string());
}

impl Pwd for Args {
    fn set_pwd(&mut self, pwd: &Path) {
        self.config_dir = Some(pwd.to_path_buf());
    }
}

pub fn ensure_directory(dir: PathBuf) -> Result<PathBuf, Error> {
    let parent = dir.parent().ok_or(Error::HomeDirNotFound)?;
    std::fs::create_dir_all(parent).map_err(|_| dir_creation_failed(parent))?;
    Ok(dir)
}

fn dir_creation_failed(p: &Path) -> Error {
    Error::DirCreationFailed {
        path: p.to_path_buf(),
    }
}

pub enum KeyType {
    Identity,
    Network,
    ContractIds,
}

impl Display for KeyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                KeyType::Identity => "identity",
                KeyType::Network => "network",
                KeyType::ContractIds => "contract-ids",
            }
        )
    }
}

impl KeyType {
    pub fn read_from_path<T: DeserializeOwned>(path: &Path) -> Result<T, Error> {
        let data = fs::read_to_string(path).map_err(|_| Error::NetworkFileRead {
            path: path.to_path_buf(),
        })?;
        Ok(toml::from_str(&data)?)
    }

    pub fn read_with_global<T: DeserializeOwned>(
        &self,
        key: &str,
        locator: &Args,
    ) -> Result<T, Error> {
        for location in locator.local_and_global()? {
            let path = self.path(location.as_ref(), key);
            match Self::read_from_path(&path) {
                Ok(t) => {
                    #[cfg(feature = "version_gte_23")]
                    if let Location::Local(_) = location {
                        print_deprecation_warning();
                    }
                    return Ok(t);
                }
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
    ) -> Result<PathBuf, Error> {
        let filepath = ensure_directory(self.path(pwd, key))?;
        let data = toml::to_string(value).map_err(|_| Error::ConfigSerialization)?;
        std::fs::write(&filepath, data).map_err(|error| Error::IdCreationFailed {
            filepath: filepath.clone(),
            error,
        })?;
        Ok(filepath)
    }

    fn root(&self, pwd: &Path) -> PathBuf {
        pwd.join(self.to_string())
    }

    fn path(&self, pwd: &Path, key: &str) -> PathBuf {
        let mut path = self.root(pwd).join(key);
        path.set_extension("toml");
        path
    }

    pub fn list_paths(&self, paths: &[Location]) -> Result<Vec<(String, Location)>, Error> {
        Ok(paths
            .iter()
            .flat_map(|p| self.list(p, true).unwrap_or_default())
            .collect())
    }

    #[cfg(feature = "version_gte_23")]
    pub fn list_paths_silent(&self, paths: &[Location]) -> Result<Vec<(String, Location)>, Error> {
        Ok(paths
            .iter()
            .flat_map(|p| self.list(p, false).unwrap_or_default())
            .collect())
    }

    #[allow(unused_variables)]
    pub fn list(
        &self,
        pwd: &Location,
        print_warning: bool,
    ) -> Result<Vec<(String, Location)>, Error> {
        let path = self.root(pwd.as_ref());
        if path.exists() {
            let mut files = self.read_dir(&path)?;
            files.sort();

            #[cfg(feature = "version_gte_23")]
            if let Location::Local(_) = pwd {
                if files.len() > 1 && print_warning {
                    print_deprecation_warning();
                }
            }

            Ok(files
                .into_iter()
                .map(|(name, p)| (name, pwd.wrap(p)))
                .collect())
        } else {
            Ok(vec![])
        }
    }

    fn read_dir(&self, dir: &Path) -> Result<Vec<(String, PathBuf)>, Error> {
        let contents = std::fs::read_dir(dir)?;
        let mut res = vec![];
        for entry in contents.filter_map(Result::ok) {
            let path = entry.path();
            let extension = match self {
                KeyType::Identity | KeyType::Network => "toml",
                KeyType::ContractIds => "json",
            };
            if let Some(ext) = path.extension().and_then(OsStr::to_str) {
                if ext == extension {
                    if let Some(os_str) = path.file_stem() {
                        res.push((os_str.to_string_lossy().trim().to_string(), path));
                    }
                }
            }
        }
        res.sort();
        Ok(res)
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
    let config_dir = if let Ok(config_home) = std::env::var("XDG_CONFIG_HOME") {
        PathBuf::from_str(&config_home).map_err(|_| Error::XdgConfigHome(config_home))?
    } else {
        UserDirs::new()
            .ok_or(Error::HomeDirNotFound)?
            .home_dir()
            .join(".config")
    };

    let soroban_dir = config_dir.join("soroban");
    let stellar_dir = config_dir.join("stellar");
    let soroban_exists = soroban_dir.exists();
    let stellar_exists = stellar_dir.exists();

    if stellar_exists && soroban_exists {
        tracing::warn!("the .stellar and .soroban config directories exist at path {config_dir:?}, using the .stellar");
    }

    if stellar_exists {
        return Ok(stellar_dir);
    }

    if soroban_exists {
        return Ok(soroban_dir);
    }

    Ok(stellar_dir)
}

// Use locator.global_config_path() to save configurations.
// This is only to be used to fetch global Stellar config (e.g. to use for defaults)
pub fn cli_config_file() -> Result<PathBuf, Error> {
    Ok(global_config_path()?.join("config.toml"))
}
