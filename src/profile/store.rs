use std::{fs::create_dir_all, fs::File, io};

use rand::Rng;
use stellar_strkey::StrkeyPrivateKeyEd25519;
use crate::network;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
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
        Self{
            current: "sandbox".to_string(),
            profiles: vec![
                (
                    "sandbox".to_string(),
                    Profile{
                        ledger_file: Some(".soroban/ledger.json".into()),
                        rpc_url: None,
                        secret_key: Some(generate_secret_key()),
                        network_passphrase: Some(network::SANDBOX_NETWORK_PASSPHRASE.to_string()),
                    }
                ),
            ],
        }
    }
}

// TODO: Generalize this to a list of key/value overrides, to be merged into the passed config as
// defaults.
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub ledger_file: Option<std::path::PathBuf>,
    pub rpc_url: Option<String>,
    pub secret_key: Option<String>,
    pub network_passphrase: Option<String>,
}

pub fn read(profiles_file: &std::path::PathBuf) -> Result<ProfilesConfig, Error> {
        // TODO: Default if file isn't found
        let mut file = match File::open(profiles_file) {
            Ok(f) => f,
            Err(e) => match e.kind() {
                io::ErrorKind::NotFound => return Ok(ProfilesConfig::default()),
                _ => return Err(Error::Io(e)),
            }
        };
        let state: ProfilesConfig = serde_json::from_reader(&mut file)?;
        Ok(state)
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
