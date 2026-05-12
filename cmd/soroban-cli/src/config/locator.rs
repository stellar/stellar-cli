use directories::UserDirs;
use itertools::Itertools;
use serde::de::DeserializeOwned;
use std::{
    ffi::OsStr,
    fmt::Display,
    fs, io,
    path::{Path, PathBuf},
    str::FromStr,
};
use stellar_strkey::{Contract, DecodeError};

use crate::{
    commands::{global, HEADING_GLOBAL},
    print::Print,
    signer::secure_store,
    utils::find_config_dir,
    xdr, Pwd,
};

use super::{
    alias,
    key::{self, Key},
    network::{self, Network},
    secret::Secret,
    utils, Config,
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    TomlSerialize(#[from] toml::ser::Error),
    #[error("Failed to find home directory")]
    HomeDirNotFound,
    #[error("Failed read current directory")]
    CurrentDirNotFound,
    #[error("Failed read current directory and no STELLAR_CONFIG_HOME is set")]
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
    #[error("STELLAR_CONFIG_HOME env variable is not a valid path. Got {0}")]
    StellarConfigDir(String),
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
    #[error("contract not found: {0}{hint}", hint = wasm_hash_hint(.0))]
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
    SecureStore(#[from] secure_store::Error),
    #[error("Only private keys and seed phrases are supported for getting private keys {0}")]
    SecretKeyOnly(String),
    #[error(transparent)]
    Key(#[from] key::Error),
    #[error("Unable to get project directory")]
    ProjectDirsError(),
    #[error(transparent)]
    InvalidName(#[from] utils::Error),
    #[error("invalid signing key or identity name")]
    InvalidSigningKey,
}

fn wasm_hash_hint(value: &str) -> &'static str {
    if value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit()) {
        "; expected a contract address (C...), got a hash"
    } else {
        ""
    }
}

#[derive(Debug, clap::Args, Default, Clone)]
#[group(skip)]
pub struct Args {
    /// Location of config directory. By default, it uses `$XDG_CONFIG_HOME/stellar` if set, falling back to `~/.config/stellar` otherwise.
    /// Contains configuration files, aliases, and other persistent settings.
    #[arg(long, global = true, help_heading = HEADING_GLOBAL)]
    pub config_dir: Option<PathBuf>,
}

#[derive(Clone)]
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
    pub fn config_dir(&self) -> Result<PathBuf, Error> {
        self.global_config_path()
    }

    pub fn local_and_global(&self) -> Result<[Location; 2], Error> {
        Ok([
            Location::Local(self.local_config()?),
            Location::Global(self.global_config_path()?),
        ])
    }

    pub fn local_config(&self) -> Result<PathBuf, Error> {
        // Always use the real process cwd for local-config discovery, regardless
        // of whether --config-dir is set.  This prevents ancestor-walking outside
        // the selected profile.
        let pwd = std::env::current_dir().map_err(|_| Error::CurrentDirNotFound)?;
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
        let path = self.global_config_path()?.join("config.toml");
        Config::load(&path)?.set_network(name).save_to(&path)
    }

    pub fn write_default_identity(&self, name: &str) -> Result<(), Error> {
        let path = self.global_config_path()?.join("config.toml");
        Config::load(&path)?.set_identity(name).save_to(&path)
    }

    pub fn write_default_inclusion_fee(&self, inclusion_fee: u32) -> Result<(), Error> {
        let path = self.global_config_path()?.join("config.toml");
        Config::load(&path)?
            .set_inclusion_fee(inclusion_fee)
            .save_to(&path)
    }

    pub fn unset_default_identity(&self) -> Result<(), Error> {
        let path = self.global_config_path()?.join("config.toml");
        Config::load(&path)?.unset_identity().save_to(&path)
    }

    pub fn unset_default_network(&self) -> Result<(), Error> {
        let path = self.global_config_path()?.join("config.toml");
        Config::load(&path)?.unset_network().save_to(&path)
    }

    pub fn unset_default_inclusion_fee(&self) -> Result<(), Error> {
        let path = self.global_config_path()?.join("config.toml");
        Config::load(&path)?.unset_inclusion_fee().save_to(&path)
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
        utils::validate_name(name)?;
        KeyType::Identity.read_with_global(name, self)
    }

    pub fn read_key(&self, key_or_name: &str) -> Result<Key, Error> {
        key_or_name
            .parse()
            .or_else(|_| self.read_identity(key_or_name))
    }

    /// Like [`Args::read_key`], but for a `SecureStore` identity loaded from disk
    /// that lacks a cached public key, derive one via the keychain (one prompt)
    /// and persist it back so subsequent reads avoid the keychain.
    pub fn read_key_with_secure_store_cache(
        &self,
        key_or_name: &str,
        hd_path: Option<u32>,
    ) -> Result<Key, Error> {
        if let Ok(literal) = key_or_name.parse::<Key>() {
            return Ok(literal);
        }
        let key = self.read_identity(key_or_name)?;
        if let Key::Secret(Secret::SecureStore {
            entry_name,
            public_key: None,
            hd_path: persisted_hd_path,
        }) = &key
        {
            // Honor the persisted hd_path when the caller passes None. Without
            // this the cache gets populated at index 0 even when the identity
            // was added with `--hd-path N`, which silently locks every later
            // read to the wrong account.
            let effective = hd_path.or(*persisted_hd_path);
            let pk = secure_store::get_public_key(entry_name, effective)?;
            let migrated = Key::Secret(Secret::SecureStore {
                entry_name: entry_name.clone(),
                public_key: Some(pk.to_string()),
                hd_path: effective,
            });
            // Best-effort write-back: if persistence fails we still return the
            // freshly-derived value so the current call succeeds.
            let _ = self.write_key(key_or_name, &migrated);
            return Ok(migrated);
        }
        Ok(key)
    }

    pub fn get_secret_key(&self, key_or_name: &str) -> Result<Secret, Error> {
        let key = self.read_key(key_or_name).map_err(|e| match e {
            Error::InvalidName(_) | Error::ConfigMissing(_, _) => Error::InvalidSigningKey,
            other => other,
        })?;
        match key {
            Key::Secret(s) => Ok(s),
            _ => Err(Error::InvalidSigningKey),
        }
    }

    /// Like [`Args::get_secret_key`], but if the secret is a `SecureStore`
    /// identity loaded from disk without a cached public key, derive it for the
    /// given `hd_path` and persist the cache. Use from signing paths so the
    /// returned `Secret` already carries the data signing needs for the hint.
    pub fn get_secret_key_with_hd_path(
        &self,
        key_or_name: &str,
        hd_path: Option<u32>,
    ) -> Result<Secret, Error> {
        let key = self
            .read_key_with_secure_store_cache(key_or_name, hd_path)
            .map_err(|e| match e {
                Error::InvalidName(_) | Error::ConfigMissing(_, _) => Error::InvalidSigningKey,
                other => other,
            })?;
        match key {
            Key::Secret(s) => Ok(s),
            _ => Err(Error::InvalidSigningKey),
        }
    }

    pub fn get_public_key(
        &self,
        key_or_name: &str,
        hd_path: Option<u32>,
    ) -> Result<xdr::MuxedAccount, Error> {
        Ok(self.read_key(key_or_name)?.muxed_account(hd_path)?)
    }

    pub fn read_network(&self, name: &str) -> Result<Network, Error> {
        utils::validate_name(name)?;
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

        if let Key::Secret(Secret::SecureStore { entry_name, .. }) = identity {
            secure_store::delete_secret(&print, &entry_name)?;
        }

        print.infoln("Removing the key's cli config file");
        KeyType::Identity.remove(name, &self.config_dir()?)
    }

    pub fn remove_network(&self, name: &str) -> Result<(), Error> {
        KeyType::Network.remove(name, &self.config_dir()?)
    }

    fn load_contract_from_alias(&self, alias: &str) -> Result<Option<alias::Data>, Error> {
        utils::validate_name(alias)?;
        let file_name = format!("{alias}.json");
        let config_dirs = self.local_and_global()?;
        let local = &config_dirs[0];
        let global = &config_dirs[1];

        match local {
            Location::Local(config_dir) => {
                if config_dir.exists() {
                    print_deprecation_warning(config_dir);
                }
            }
            Location::Global(_) => unreachable!(),
        }

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
        utils::validate_name(alias)?;
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

        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            std::fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(dir)
                .map_err(|_| Error::CannotAccessConfigDir)?;
        }

        #[cfg(not(unix))]
        std::fs::create_dir_all(dir).map_err(|_| Error::CannotAccessConfigDir)?;

        let content = fs::read_to_string(&path).unwrap_or_default();
        let mut data: alias::Data = serde_json::from_str(&content).unwrap_or_default();

        data.ids
            .insert(network_passphrase.into(), contract_id.to_string());

        let content = serde_json::to_string(&data)?;
        write_hardened_file(&path, content.as_bytes())?;

        #[cfg(unix)]
        if let Ok(root) = self.config_dir() {
            fix_config_permissions(root);
        }

        Ok(())
    }

    pub fn remove_contract_id(&self, network_passphrase: &str, alias: &str) -> Result<(), Error> {
        let path = self.alias_path(alias)?;

        if !path.is_file() {
            return Err(Error::CannotAccessAliasConfigFile);
        }

        let content = fs::read_to_string(&path).unwrap_or_default();
        let mut data: alias::Data = serde_json::from_str(&content).unwrap_or_default();

        data.ids.remove::<str>(network_passphrase);

        let content = serde_json::to_string(&data)?;
        write_hardened_file(&path, content.as_bytes())?;
        Ok(())
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
        if let Some(config_dir) = &self.config_dir {
            return Ok(config_dir.clone());
        }

        global_config_path()
    }
}

pub fn print_deprecation_warning(dir: &Path) {
    let print = Print::new(false);
    let Ok(global_dir) = global_config_path() else {
        return;
    };
    let global_dir = fs::canonicalize(&global_dir).unwrap_or(global_dir);

    // No warning if local and global dirs are the same (e.g., both set to STELLAR_CONFIG_HOME)
    if dir == global_dir {
        return;
    }

    print.warnln(format!(
        "A local config was found at {dir:?} but is no longer read."
    ));
    print.blankln(format!(
        " Run `stellar config migrate` to move the local config into the global config ({global_dir:?})."
    ));
}

impl Pwd for Args {
    fn set_pwd(&mut self, pwd: &Path) {
        self.config_dir = Some(pwd.to_path_buf());
    }
}

#[cfg(unix)]
fn fix_config_permissions(root: std::path::PathBuf) {
    use std::os::unix::fs::PermissionsExt;

    let mut bad_dirs = Vec::new();
    let mut bad_files = Vec::new();
    let mut stack = vec![root];

    while let Some(dir) = stack.pop() {
        if let Ok(meta) = std::fs::metadata(&dir) {
            if meta.permissions().mode() & 0o777 != 0o700 {
                bad_dirs.push(dir.clone());
            }
        }

        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.filter_map(Result::ok) {
                let path = entry.path();

                if path.is_dir() {
                    stack.push(path);
                } else if let Ok(meta) = std::fs::metadata(&path) {
                    if meta.permissions().mode() & 0o777 != 0o600 {
                        bad_files.push(path);
                    }
                }
            }
        }
    }

    let print = Print::new(false);

    if !bad_dirs.is_empty() {
        print.warnln("Updated config directories permissions to 0700.");

        for dir in bad_dirs {
            let _ = set_hardened_permissions(&dir);
        }
    }

    if !bad_files.is_empty() {
        print.warnln("Updated config files permissions to 0600.");

        for file in bad_files {
            let _ = set_hardened_permissions(&file);
        }
    }
}

#[allow(unused_variables, clippy::unnecessary_wraps)]
pub(crate) fn set_hardened_permissions(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = if path.is_dir() { 0o700 } else { 0o600 };
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))?;
    }
    Ok(())
}

/// Writes `contents` to `path`, creating the file with `0600` on Unix and
/// resetting the mode to exactly `0600` afterwards regardless of any
/// pre-existing permissions. Falls back to `std::fs::write` on non-Unix
/// platforms.
pub(crate) fn write_hardened_file(path: &Path, contents: &[u8]) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::io::Write as _;
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)?;
        file.write_all(contents)?;
        set_hardened_permissions(path)?;
    }

    #[cfg(not(unix))]
    std::fs::write(path, contents)?;

    Ok(())
}

pub fn ensure_directory(dir: PathBuf) -> Result<PathBuf, Error> {
    let parent = dir.parent().ok_or(Error::HomeDirNotFound)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(parent)
            .map_err(|_| dir_creation_failed(parent))?;
        fix_config_permissions(parent.to_path_buf());
    }

    #[cfg(not(unix))]
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
        Ok(self.read_with_global_with_location(key, locator)?.0)
    }

    pub fn read_with_global_with_location<T: DeserializeOwned>(
        &self,
        key: &str,
        locator: &Args,
    ) -> Result<(T, Location), Error> {
        for location in locator.local_and_global()? {
            match &location {
                Location::Local(config_dir) => {
                    if config_dir.exists() {
                        print_deprecation_warning(config_dir);
                    }
                    continue;
                }
                Location::Global(_) => {}
            }

            let path = self.path(location.as_ref(), key);
            if let Ok(t) = Self::read_from_path(&path) {
                return Ok((t, location));
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
        write_hardened_file(&filepath, data.as_bytes()).map_err(|error| {
            Error::IdCreationFailed {
                filepath: filepath.clone(),
                error,
            }
        })?;

        #[cfg(unix)]
        fix_config_permissions(pwd.to_path_buf());

        Ok(filepath)
    }

    fn root(&self, pwd: &Path) -> PathBuf {
        pwd.join(self.to_string())
    }

    pub fn path(&self, pwd: &Path, key: &str) -> PathBuf {
        let mut path = self.root(pwd).join(key);
        match self {
            KeyType::Identity | KeyType::Network => path.set_extension("toml"),
            KeyType::ContractIds => path.set_extension("json"),
        };
        path
    }

    pub fn list_paths(&self, paths: &[Location]) -> Result<Vec<(String, Location)>, Error> {
        Ok(paths
            .iter()
            .filter(|p| {
                if let Location::Local(dir) = p {
                    if dir.exists() {
                        print_deprecation_warning(dir);
                    }
                    return false;
                }
                true
            })
            .unique_by(|p| location_to_string(p))
            .flat_map(|p| self.list(p, false).unwrap_or_default())
            .collect())
    }

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

            if let Location::Local(config_dir) = pwd {
                if files.len() > 1 && print_warning {
                    print_deprecation_warning(config_dir);
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
    if let Ok(config_home) = std::env::var("STELLAR_CONFIG_HOME") {
        return PathBuf::from_str(&config_home).map_err(|_| Error::StellarConfigDir(config_home));
    }

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

fn location_to_string(location: &Location) -> String {
    match location {
        Location::Local(p) | Location::Global(p) => fs::canonicalize(AsRef::<Path>::as_ref(p))
            .unwrap_or(p.clone())
            .display()
            .to_string(),
    }
}

// Use locator.global_config_path() to save configurations.
// This is only to be used to fetch global Stellar config (e.g. to use for defaults)
pub fn cli_config_file() -> Result<PathBuf, Error> {
    Ok(global_config_path()?.join("config.toml"))
}

#[cfg(test)]
mod error_message_tests {
    use super::*;

    #[test]
    fn contract_not_found_plain_alias_has_no_hint() {
        let err = Error::ContractNotFound("alice".to_string());
        assert_eq!(err.to_string(), "contract not found: alice");
    }

    #[test]
    fn contract_not_found_64_char_lowercase_hex_includes_wasm_hash_hint() {
        let hash = "5ea0f3d6c880148c8da088809e851732127fc36b7b42bbdde6052fcc6f6253f3";
        let err = Error::ContractNotFound(hash.to_string());
        assert_eq!(
            err.to_string(),
            format!("contract not found: {hash}; expected a contract address (C...), got a hash"),
        );
    }

    #[test]
    fn contract_not_found_64_char_uppercase_hex_includes_wasm_hash_hint() {
        let hash = "5EA0F3D6C880148C8DA088809E851732127FC36B7B42BBDDE6052FCC6F6253F3";
        let err = Error::ContractNotFound(hash.to_string());
        assert!(
            err.to_string().contains("got a hash"),
            "expected wasm-hash hint for uppercase hex, got: {err}",
        );
    }

    #[test]
    fn contract_not_found_64_char_mixed_case_hex_includes_wasm_hash_hint() {
        let hash = "5ea0F3d6C880148c8DA088809e851732127fc36b7b42BBDDE6052fcc6F6253F3";
        let err = Error::ContractNotFound(hash.to_string());
        assert!(
            err.to_string().contains("got a hash"),
            "expected wasm-hash hint for mixed-case hex, got: {err}",
        );
    }

    #[test]
    fn contract_not_found_short_hex_string_has_no_hint() {
        let err = Error::ContractNotFound("deadbeef".to_string());
        assert_eq!(err.to_string(), "contract not found: deadbeef");
    }

    #[test]
    fn contract_not_found_64_char_non_hex_has_no_hint() {
        let value = "z".repeat(64);
        let err = Error::ContractNotFound(value.clone());
        assert_eq!(err.to_string(), format!("contract not found: {value}"));
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::collections::HashMap;

    #[test]
    fn overwrite_resets_file_permissions_to_0600() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let identity_dir = dir.path().join("identity");
        std::fs::create_dir_all(&identity_dir).unwrap();

        // Pre-create alice.toml at 0644 to simulate an inherited insecure mode.
        let alice = identity_dir.join("alice.toml");
        std::fs::write(&alice, "seed_phrase = \"old\"\n").unwrap();
        std::fs::set_permissions(&alice, std::fs::Permissions::from_mode(0o644)).unwrap();

        assert_eq!(
            std::fs::metadata(&alice).unwrap().permissions().mode() & 0o777,
            0o644,
            "setup: alice.toml should start at 0644"
        );

        let value: HashMap<String, String> = HashMap::new();
        KeyType::Identity
            .write("alice", &value, dir.path())
            .unwrap();

        let perms = std::fs::metadata(&alice).unwrap().permissions();
        assert_eq!(
            perms.mode() & 0o777,
            0o600,
            "overwritten identity file should be 0600, got {:o}",
            perms.mode() & 0o777
        );
    }

    #[test]
    fn test_write_sets_file_permissions_to_0600() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let value: HashMap<String, String> = HashMap::new();
        let path = KeyType::Identity
            .write("test-key", &value, dir.path())
            .unwrap();

        let perms = std::fs::metadata(&path).unwrap().permissions();

        assert_eq!(
            perms.mode() & 0o777,
            0o600,
            "identity file should be owner-only readable (0600), got {:o}",
            perms.mode() & 0o777
        );
    }

    #[test]
    fn test_ensure_directory_sets_dir_permissions_to_0700() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("sub").join("file.toml");
        ensure_directory(target).unwrap();

        let perms = std::fs::metadata(dir.path().join("sub"))
            .unwrap()
            .permissions();

        assert_eq!(
            perms.mode() & 0o777,
            0o700,
            "identity directory should be owner-only (0700), got {:o}",
            perms.mode() & 0o777
        );
    }

    use crate::test_utils::{with_cwd_guard, with_env_guard};

    #[test]
    #[serial]
    fn local_config_identity_is_not_read() {
        use crate::config::key::Key;

        let tmp = tempfile::tempdir().unwrap();

        with_env_guard(&["STELLAR_CONFIG_HOME", "XDG_CONFIG_HOME"], || {
            with_cwd_guard(|| {
                let local_identity_dir = tmp.path().join(".stellar/identity");
                std::fs::create_dir_all(&local_identity_dir).unwrap();
                std::fs::write(
                    local_identity_dir.join("alice.toml"),
                    "seed_phrase = \"abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about\"\n",
                )
                .unwrap();

                let global_cfg = tmp.path().join("global");
                std::fs::create_dir_all(&global_cfg).unwrap();
                std::env::set_var("STELLAR_CONFIG_HOME", &global_cfg);

                std::env::set_current_dir(tmp.path()).unwrap();

                let locator = Args { config_dir: None };
                let result = locator.read_identity("alice");
                assert!(
                    result.is_err(),
                    "local config identity should not be read, but got: {:?}",
                    result.map(|k: Key| format!("{k:?}"))
                );
            });
        });
    }

    #[test]
    #[serial]
    fn local_config_contract_alias_is_not_read() {
        let tmp = tempfile::tempdir().unwrap();

        with_env_guard(&["STELLAR_CONFIG_HOME", "XDG_CONFIG_HOME"], || {
            with_cwd_guard(|| {
                let local_alias_dir = tmp.path().join(".stellar/contract-ids");
                std::fs::create_dir_all(&local_alias_dir).unwrap();
                std::fs::write(
                    local_alias_dir.join("mycontract.json"),
                    r#"{"ids":{"testnet":"CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4"}}"#,
                )
                .unwrap();

                let global_cfg = tmp.path().join("global");
                std::fs::create_dir_all(&global_cfg).unwrap();
                std::env::set_var("STELLAR_CONFIG_HOME", &global_cfg);

                std::env::set_current_dir(tmp.path()).unwrap();

                let locator = Args { config_dir: None };
                let result = locator.load_contract_from_alias("mycontract").unwrap();
                assert!(
                    result.is_none(),
                    "local config contract alias should not be read"
                );
            });
        });
    }

    #[test]
    #[serial]
    fn local_config_identity_not_listed() {
        let tmp = tempfile::tempdir().unwrap();

        with_env_guard(&["STELLAR_CONFIG_HOME", "XDG_CONFIG_HOME"], || {
            with_cwd_guard(|| {
                let local_identity_dir = tmp.path().join(".stellar/identity");
                std::fs::create_dir_all(&local_identity_dir).unwrap();
                std::fs::write(
                    local_identity_dir.join("alice.toml"),
                    "seed_phrase = \"abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about\"\n",
                )
                .unwrap();

                let global_cfg = tmp.path().join("global");
                std::fs::create_dir_all(&global_cfg).unwrap();
                std::env::set_var("STELLAR_CONFIG_HOME", &global_cfg);

                std::env::set_current_dir(tmp.path()).unwrap();

                let locator = Args { config_dir: None };
                let identities = locator.list_identities().unwrap();
                assert!(
                    !identities.contains(&"alice".to_string()),
                    "local config identities should not appear in list, got: {identities:?}"
                );
            });
        });
    }

    #[test]
    #[serial]
    fn local_config_network_is_not_read() {
        let tmp = tempfile::tempdir().unwrap();

        with_env_guard(&["STELLAR_CONFIG_HOME", "XDG_CONFIG_HOME"], || {
            with_cwd_guard(|| {
                let local_network_dir = tmp.path().join(".stellar/network");
                std::fs::create_dir_all(&local_network_dir).unwrap();
                std::fs::write(
                    local_network_dir.join("mynet.toml"),
                    "rpc_url = \"https://127.0.0.1\"\nnetwork_passphrase = \"Local\"\n",
                )
                .unwrap();

                let global_cfg = tmp.path().join("global");
                std::fs::create_dir_all(&global_cfg).unwrap();
                std::env::set_var("STELLAR_CONFIG_HOME", &global_cfg);

                std::env::set_current_dir(tmp.path()).unwrap();

                let locator = Args { config_dir: None };
                let result = locator.read_network("mynet");
                assert!(result.is_err(), "local config network should not be read");
            });
        });
    }

    #[test]
    #[serial]
    fn local_config_network_not_listed() {
        let tmp = tempfile::tempdir().unwrap();

        with_env_guard(&["STELLAR_CONFIG_HOME", "XDG_CONFIG_HOME"], || {
            with_cwd_guard(|| {
                let local_network_dir = tmp.path().join(".stellar/network");
                std::fs::create_dir_all(&local_network_dir).unwrap();
                std::fs::write(
                    local_network_dir.join("mynet.toml"),
                    "rpc_url = \"https://127.0.0.1\"\nnetwork_passphrase = \"Local\"\n",
                )
                .unwrap();

                let global_cfg = tmp.path().join("global");
                std::fs::create_dir_all(&global_cfg).unwrap();
                std::env::set_var("STELLAR_CONFIG_HOME", &global_cfg);

                std::env::set_current_dir(tmp.path()).unwrap();

                let locator = Args { config_dir: None };
                let networks = locator.list_networks().unwrap();
                assert!(
                    !networks.contains(&"mynet".to_string()),
                    "local config networks should not appear in list, got: {networks:?}"
                );
            });
        });
    }

    #[test]
    #[serial]
    fn local_config_contract_alias_not_listed() {
        let tmp = tempfile::tempdir().unwrap();

        with_env_guard(&["STELLAR_CONFIG_HOME", "XDG_CONFIG_HOME"], || {
            with_cwd_guard(|| {
                let local_alias_dir = tmp.path().join(".stellar/contract-ids");
                std::fs::create_dir_all(&local_alias_dir).unwrap();
                std::fs::write(
                    local_alias_dir.join("mycontract.json"),
                    r#"{"ids":{"testnet":"CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4"}}"#,
                )
                .unwrap();

                let global_cfg = tmp.path().join("global");
                std::fs::create_dir_all(&global_cfg).unwrap();
                std::env::set_var("STELLAR_CONFIG_HOME", &global_cfg);

                std::env::set_current_dir(tmp.path()).unwrap();

                let locator = Args { config_dir: None };
                let [local, global] = locator.local_and_global().unwrap();

                // Verify the alias ls logic: local must be skipped, global has no aliases.
                assert!(matches!(local, Location::Local(_)));
                assert!(matches!(global, Location::Global(_)));
                let global_alias_dir = global.as_ref().join("contract-ids");
                assert!(
                    !global_alias_dir.exists(),
                    "global alias dir should be empty — local alias must not bleed through"
                );
            });
        });
    }

    #[test]
    #[serial]
    fn config_dir_does_not_search_ancestors_for_identity() {
        // Regression test for: --config-dir ancestor search discloses secrets
        // outside the selected profile (security finding 004).
        //
        // Place alice.toml in an ancestor of the explicit --config-dir.
        // The command should fail to find alice, not read the ancestor file.
        use crate::config::key::Key;

        let tmp = tempfile::tempdir().unwrap();

        with_env_guard(&["STELLAR_CONFIG_HOME", "XDG_CONFIG_HOME"], || {
            // Ancestor .stellar with alice.toml
            let ancestor_identity_dir = tmp.path().join(".stellar/identity");
            std::fs::create_dir_all(&ancestor_identity_dir).unwrap();
            std::fs::write(
                ancestor_identity_dir.join("alice.toml"),
                "seed_phrase = \"abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about\"\n",
            )
            .unwrap();

            // Explicit --config-dir is a descendant of the ancestor, and is empty.
            let isolated = tmp.path().join("sub/deep");
            std::fs::create_dir_all(&isolated).unwrap();

            // Global config is also separate and empty.
            let global_cfg = tmp.path().join("global-cfg");
            std::fs::create_dir_all(&global_cfg).unwrap();
            std::env::set_var("STELLAR_CONFIG_HOME", &global_cfg);

            let locator = Args {
                config_dir: Some(isolated),
            };

            let result = locator.read_identity("alice");
            assert!(
                result.is_err(),
                "expected error when alice is absent from --config-dir and global, \
                 but got: {:?}",
                result.map(|k: Key| format!("{k:?}"))
            );
        });
    }

    #[test]
    #[serial]
    fn test_print_deprecation_warning_no_panic_when_global_dir_missing() {
        let tmp = tempfile::tempdir().unwrap();

        with_env_guard(&["STELLAR_CONFIG_HOME", "XDG_CONFIG_HOME", "HOME"], || {
            let fake_home = tmp.path().join("home");
            std::fs::create_dir_all(&fake_home).unwrap();
            std::env::set_var("HOME", &fake_home);

            let local_dir = tmp.path().join("workdir/.stellar");
            std::fs::create_dir_all(&local_dir).unwrap();

            // Must not panic even though ~/.config/stellar does not exist
            print_deprecation_warning(&local_dir);
        });
    }

    mod secure_store_cache {
        use super::super::*;

        const TEST_PUBLIC_KEY: &str = "GAREAZZQWHOCBJS236KIE3AWYBVFLSBK7E5UW3ICI3TCRWQKT5LNLCEZ";
        const TEST_SECRET_KEY: &str = "SBF5HLRREHMS36XZNTUSKZ6FTXDZGNXOHF4EXKUL5UCWZLPBX3NGJ4BH";

        fn locator_with_tempdir() -> (tempfile::TempDir, Args) {
            let dir = tempfile::tempdir().unwrap();
            let args = Args {
                config_dir: Some(dir.path().to_path_buf()),
            };
            (dir, args)
        }

        // The legacy-file -> derive-via-keychain -> write-back path is
        // exercised end-to-end by the soroban-test integration test
        // `secure_store_key_management`. The keyring crate's mock builder
        // assigns each `Entry` instance its own in-memory credential
        // (CredentialPersistence::EntryOnly), which makes the read-after-write
        // round trip impossible to simulate in pure unit tests.

        #[test]
        fn passes_through_already_cached_identity_without_keychain_access() {
            let (_dir, locator) = locator_with_tempdir();

            // Entry name points to a non-existent keychain entry, so any
            // keychain access would fail the test.
            let already = Secret::SecureStore {
                entry_name: "secure_store:org.stellar.cli-no-such-entry".to_string(),
                public_key: Some(TEST_PUBLIC_KEY.to_string()),
                hd_path: None,
            };
            locator.write_identity("already", &already).unwrap();

            let key = locator
                .read_key_with_secure_store_cache("already", None)
                .unwrap();
            match key {
                Key::Secret(Secret::SecureStore {
                    public_key: Some(pk),
                    ..
                }) => assert_eq!(pk, TEST_PUBLIC_KEY),
                other => panic!("expected SecureStore, got {other:?}"),
            }
        }

        #[test]
        fn passes_through_non_secure_store_identity() {
            let (_dir, locator) = locator_with_tempdir();

            let secret = Secret::SecretKey {
                secret_key: TEST_SECRET_KEY.to_string(),
            };
            locator.write_identity("plain", &secret).unwrap();

            let key = locator
                .read_key_with_secure_store_cache("plain", None)
                .unwrap();
            assert!(matches!(
                key,
                Key::Secret(Secret::SecretKey { ref secret_key }) if secret_key == TEST_SECRET_KEY
            ));
        }

        #[test]
        fn returns_literal_public_key_without_disk_lookup() {
            let (_dir, locator) = locator_with_tempdir();

            let key = locator
                .read_key_with_secure_store_cache(TEST_PUBLIC_KEY, None)
                .unwrap();
            assert!(matches!(key, Key::PublicKey(_)));
        }
    }
}
