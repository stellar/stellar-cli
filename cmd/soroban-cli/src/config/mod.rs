use clap::{arg, command};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::Write,
};

use crate::{
    signer,
    xdr::{self, SequenceNumber, Transaction, TransactionEnvelope, TransactionV1Envelope, VecM},
    Pwd,
};

use crate::commands::global;
use crate::commands::keys::generate;
use crate::config::address::KeyName;
use network::Network;

pub mod address;
pub mod alias;
pub mod data;
pub mod key;
pub mod locator;
pub mod network;
pub mod sc_address;
pub mod secret;
pub mod sign_with;
pub mod upgrade_check;

pub use address::UnresolvedMuxedAccount;
pub use alias::UnresolvedContract;
pub use sc_address::UnresolvedScAddress;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Network(#[from] network::Error),
    #[error(transparent)]
    Secret(#[from] secret::Error),
    #[error(transparent)]
    Config(#[from] locator::Error),
    #[error(transparent)]
    Rpc(#[from] soroban_rpc::Error),
    #[error(transparent)]
    Signer(#[from] signer::Error),
    #[error(transparent)]
    SignWith(#[from] sign_with::Error),
    #[error(transparent)]
    StellarStrkey(#[from] stellar_strkey::DecodeError),
    #[error(transparent)]
    Address(#[from] address::Error),
}

#[derive(Debug, clap::Args, Clone, Default)]
#[group(skip)]
pub struct Args {
    #[command(flatten)]
    pub network: network::Args,

    #[arg(
        long,
        short = 's',
        visible_alias = "source",
        env = "STELLAR_ACCOUNT",
        default_value = "default"
    )]
    /// Account that where transaction originates from. Alias `source`.
    /// Can be an identity (--source alice), a public key (--source GDKW...),
    /// a muxed account (--source MDA…), a secret key (--source SC36…),
    /// or a seed phrase (--source "kite urban…").
    /// If `--build-only` or `--sim-only` flags were NOT provided, this key will also be used to
    /// sign the final transaction. In that case, trying to sign with public key will fail.
    pub source_account: Option<UnresolvedMuxedAccount>,

    #[command(flatten)]
    pub locator: locator::Args,

    #[command(flatten)]
    pub sign_with: sign_with::Args,
}

impl Args {
    // TODO: Replace PublicKey with MuxedAccount once https://github.com/stellar/rs-stellar-xdr/pull/396 is merged.
    pub async fn source_account(&self) -> Result<xdr::MuxedAccount, Error> {
        match &self.source_account {
            Some(UnresolvedMuxedAccount::AliasOrSecret(alias)) if alias == "default" => {
                let default_name = KeyName("default".to_string());
                let network = self.network.get(&self.locator)?;
                let should_fund = network.network_passphrase == network::passphrase::TESTNET;
                let generate_cmd = generate::Cmd {
                    name: default_name.clone(),
                    // #[cfg(feature = "version_lt_23")]
                    // no_fund: !should_fund,
                    seed: None, // Random seed for security
                    as_secret: false,
                    secure_store: false,
                    config_locator: self.locator.clone(),
                    hd_path: None,
                    network: self.network.clone(),
                    fund: should_fund,
                    overwrite: true,
                };
                let _ = generate_cmd
                    .run(&global::Args {
                        quiet: true,
                        ..Default::default()
                    })
                    .await;
                Ok(UnresolvedMuxedAccount::AliasOrSecret("default".to_string())
                    .resolve_muxed_account(&self.locator, self.hd_path())
                    .await?)
            }
            _ => Ok(self
                .source_account
                .as_ref()
                .unwrap()
                .resolve_muxed_account(&self.locator, self.hd_path())
                .await?),
        }
    }

    pub fn key_pair(&self) -> Result<ed25519_dalek::SigningKey, Error> {
        let key = &self
            .source_account
            .as_ref()
            .unwrap()
            .resolve_secret(&self.locator)?;
        Ok(key.key_pair(self.hd_path())?)
    }

    pub async fn sign(&self, tx: Transaction) -> Result<TransactionEnvelope, Error> {
        let tx_env = TransactionEnvelope::Tx(TransactionV1Envelope {
            tx,
            signatures: VecM::default(),
        });
        Ok(self
            .sign_with
            .sign_tx_env(
                &tx_env,
                &self.locator,
                &self.network.get(&self.locator)?,
                false,
                Some(&self.source_account.as_ref().unwrap()),
            )
            .await?)
    }

    pub async fn sign_soroban_authorizations(
        &self,
        tx: &Transaction,
        signers: &[ed25519_dalek::SigningKey],
    ) -> Result<Option<Transaction>, Error> {
        let network = self.get_network()?;
        let source_key = self.key_pair()?;
        let client = network.rpc_client()?;
        let latest_ledger = client.get_latest_ledger().await?.sequence;
        let seq_num = latest_ledger + 60; // ~ 5 min
        Ok(signer::sign_soroban_authorizations(
            tx,
            &source_key,
            signers,
            seq_num,
            &network.network_passphrase,
        )?)
    }

    pub fn get_network(&self) -> Result<Network, Error> {
        Ok(self.network.get(&self.locator)?)
    }

    pub async fn next_sequence_number(
        &self,
        account: impl Into<xdr::AccountId>,
    ) -> Result<SequenceNumber, Error> {
        let network = self.get_network()?;
        let client = network.rpc_client()?;
        Ok((client
            .get_account(&account.into().to_string())
            .await?
            .seq_num
            .0
            + 1)
        .into())
    }

    pub fn hd_path(&self) -> Option<usize> {
        self.sign_with.hd_path
    }
}

impl Pwd for Args {
    fn set_pwd(&mut self, pwd: &std::path::Path) {
        self.locator.set_pwd(pwd);
    }
}

#[derive(Debug, clap::Args, Clone, Default)]
#[group(skip)]
pub struct ArgsLocatorAndNetwork {
    #[command(flatten)]
    pub network: network::Args,

    #[command(flatten)]
    pub locator: locator::Args,
}

impl ArgsLocatorAndNetwork {
    pub fn get_network(&self) -> Result<Network, Error> {
        Ok(self.network.get(&self.locator)?)
    }
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct Config {
    pub defaults: Defaults,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct Defaults {
    pub network: Option<String>,
    pub identity: Option<String>,
}

impl Config {
    pub fn new() -> Result<Config, locator::Error> {
        let path = locator::config_file()?;

        if path.exists() {
            let data = fs::read_to_string(&path).map_err(|_| locator::Error::FileRead { path })?;
            Ok(toml::from_str(&data)?)
        } else {
            Ok(Config::default())
        }
    }

    #[must_use]
    pub fn set_network(mut self, s: &str) -> Self {
        self.defaults.network = Some(s.to_string());
        self
    }

    #[must_use]
    pub fn set_identity(mut self, s: &str) -> Self {
        self.defaults.identity = Some(s.to_string());
        self
    }

    pub fn save(&self) -> Result<(), locator::Error> {
        let toml_string = toml::to_string(&self)?;
        let path = locator::config_file()?;
        // Depending on the platform, this function may fail if the full directory path does not exist
        let mut file = File::create(locator::ensure_directory(path)?)?;
        file.write_all(toml_string.as_bytes())?;

        Ok(())
    }
}
