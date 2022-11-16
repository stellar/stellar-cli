use std::{fs::create_dir_all, fs::File, io};

use crate::{network, HEADING_SANDBOX, HEADING_RPC};
use rand::Rng;
use stellar_strkey::StrkeyPrivateKeyEd25519;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
    #[error("profile not found: {name}")]
    ProfileNotFound { name: String },
    #[error("cannot parse secret key")]
    CannotParseSecretKey,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfilesConfig {
    pub current: String,
    pub profiles: Vec<(String, Profile)>,
}

pub fn generate_secret_key() -> String {
    let bytes: [u8; 32] = rand::thread_rng().gen::<[u8; 32]>();
    StrkeyPrivateKeyEd25519(bytes).to_string()
}

impl Default for ProfilesConfig {
    fn default() -> Self {
        Self {
            current: "sandbox".to_string(),
            profiles: vec![(
                "sandbox".to_string(),
                Profile {
                    ledger_file: ".soroban/ledger.json".into(),
                    rpc_url: None,
                    secret_key: Some(generate_secret_key()),
                    network_passphrase: Some(network::SANDBOX_NETWORK_PASSPHRASE.to_string()),
                },
            )],
        }
    }
}

// TODO: Generalize this to a list of key/value overrides, to be merged into the passed config as
// defaults.
#[derive(serde::Serialize, serde::Deserialize, Clone, clap::Args, Default, Eq, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    /// File to persist ledger state (if using the sandbox)
    #[clap(
    long,
    parse(from_os_str),
    default_value = ".soroban/ledger.json",
    conflicts_with = "rpc-url",
    env = "SOROBAN_LEDGER_FILE",
    help_heading = HEADING_SANDBOX,
)]
    pub ledger_file: std::path::PathBuf,

    /// RPC server endpoint
    #[clap(
    long,
    requires = "network-passphrase",
    env = "SOROBAN_RPC_URL",
    help_heading = HEADING_RPC,
)]
    pub rpc_url: Option<String>,
    /// Secret key to sign the transaction sent to the rpc server
    #[clap(
    long = "secret-key",
    env = "SOROBAN_SECRET_KEY",
    help_heading = HEADING_RPC,
)]
    pub secret_key: Option<String>,
    /// Network passphrase to sign the transaction sent to the rpc server
    #[clap(
    long = "network-passphrase",
    env = "SOROBAN_NETWORK_PASSPHRASE",
    help_heading = HEADING_RPC,
)]
    pub network_passphrase: Option<String>,
}

impl Profile {
    pub fn parse_secret_key(&self) -> Result<[u8; 32], Error> {
        let seed = match &self.secret_key {
            None => Err(Error::CannotParseSecretKey),
            Some(strkey) => Ok(StrkeyPrivateKeyEd25519::from_string(strkey)
                .map_err(|_| Error::CannotParseSecretKey)?),
        }?;
        Ok(seed.0)
    }

    pub fn parse_secret_key_dalek(&self) -> Result<ed25519_dalek::Keypair, Error> {
        let seed = self.parse_secret_key()?;
        let secret_key =
            ed25519_dalek::SecretKey::from_bytes(&seed).map_err(|_| Error::CannotParseSecretKey)?;
        let public_key = (&secret_key).into();
        Ok(ed25519_dalek::Keypair {
            secret: secret_key,
            public: public_key,
        })
    }
}

pub fn read(profiles_file: &std::path::PathBuf) -> Result<ProfilesConfig, Error> {
    let mut file = match File::open(profiles_file) {
        Ok(f) => f,
        Err(e) => match e.kind() {
            io::ErrorKind::NotFound => return Ok(ProfilesConfig::default()),
            _ => return Err(Error::Io(e)),
        },
    };
    let state: ProfilesConfig = serde_json::from_reader(&mut file)?;
    Ok(state)
}

pub fn read_current(
    profiles_file: &std::path::PathBuf,
    selected: Option<String>,
) -> Result<Profile, Error> {
    let state = read(profiles_file)?;
    let needle = selected.unwrap_or(state.current);
    for (name, p) in &state.profiles {
        if name == &needle {
            return Ok(p.clone());
        }
    }
    Err(Error::ProfileNotFound { name: needle })
}

pub fn commit(profiles_file: &std::path::PathBuf, new_state: &ProfilesConfig) -> Result<(), Error> {
    if let Some(dir) = profiles_file.parent() {
        if !dir.exists() {
            create_dir_all(dir)?;
        }
    }

    let file = File::create(profiles_file)?;
    serde_json::to_writer(&file, &new_state)?;

    Ok(())
}
