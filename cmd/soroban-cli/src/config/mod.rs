use clap::{arg, command};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::Write,
};

use crate::{
    print::Print,
    signer::{self, Signer},
    xdr::{self, SequenceNumber, Transaction, TransactionEnvelope, TransactionV1Envelope, VecM},
    Pwd,
};
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

use crate::config::locator::cli_config_file;
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
    Locator(#[from] locator::Error),
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

    #[arg(long, short = 's', visible_alias = "source", env = "STELLAR_ACCOUNT")]
    /// Account that where transaction originates from. Alias `source`.
    /// Can be an identity (--source alice), a public key (--source GDKW...),
    /// a muxed account (--source MDA…), a secret key (--source SC36…),
    /// or a seed phrase (--source "kite urban…").
    /// If `--build-only` was NOT provided, this key will also be used to
    /// sign the final transaction. In that case, trying to sign with public key will fail.
    pub source_account: UnresolvedMuxedAccount,

    #[command(flatten)]
    pub locator: locator::Args,

    #[command(flatten)]
    pub sign_with: sign_with::Args,

    /// ⚠️ Deprecated, use `--inclusion-fee`. Fee amount for transaction, in stroops. 1 stroop = 0.0000001 xlm
    #[arg(long, env = "STELLAR_FEE")]
    pub fee: Option<u32>,

    /// Maximum fee amount for transaction inclusion, in stroops. 1 stroop = 0.0000001 xlm. Defaults to 100 if no arg, env, or config value is provided
    #[arg(long, env = "STELLAR_INCLUSION_FEE")]
    pub inclusion_fee: Option<u32>,
}

impl Args {
    // TODO: Replace PublicKey with MuxedAccount once https://github.com/stellar/rs-stellar-xdr/pull/396 is merged.
    pub async fn source_account(&self) -> Result<xdr::MuxedAccount, Error> {
        Ok(self
            .source_account
            .resolve_muxed_account(&self.locator, self.hd_path())
            .await?)
    }

    pub async fn source_signer(&self) -> Result<Signer, Error> {
        let print = Print::new(true);
        let secret = &self.source_account.resolve_secret(&self.locator)?;
        Ok(secret.signer(None, print).await?)
    }

    pub fn key_pair(&self) -> Result<ed25519_dalek::SigningKey, Error> {
        let key = &self.source_account.resolve_secret(&self.locator)?;
        Ok(key.key_pair(self.hd_path())?)
    }

    pub async fn sign(&self, tx: Transaction, quiet: bool) -> Result<TransactionEnvelope, Error> {
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
                quiet,
                Some(&self.source_account),
            )
            .await?)
    }

    pub async fn sign_soroban_authorizations(
        &self,
        tx: &Transaction,
        signers: &[Signer],
    ) -> Result<Option<Transaction>, Error> {
        let network = self.get_network()?;
        let source_signer = self.source_signer().await?;
        let client = network.rpc_client()?;
        let latest_ledger = client.get_latest_ledger().await?.sequence;
        let seq_num = latest_ledger + 60; // ~ 5 min
        Ok(signer::sign_soroban_authorizations(
            tx,
            &source_signer,
            signers,
            seq_num,
            &network.network_passphrase,
        )?)
    }

    pub fn get_network(&self) -> Result<Network, Error> {
        Ok(self.network.get(&self.locator)?)
    }

    /// Get the inclusion fee if available from args, otherwise fall back to fee,
    /// and finally return 100 if nothing is set.
    ///
    /// Precedence is:
    /// 1. inclusion_fee (via clap, arg then env var)
    /// 2. fee (via clap, arg then env var)
    /// 3. default of 100 stroops
    pub fn get_inclusion_fee(&self) -> Result<u32, Error> {
        Ok(self.inclusion_fee.or(self.fee).unwrap_or(100))
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
    pub inclusion_fee: Option<u32>,
}

impl Config {
    pub fn new() -> Result<Config, locator::Error> {
        let path = cli_config_file()?;

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

    #[must_use]
    pub fn set_inclusion_fee(mut self, uint: Option<u32>) -> Self {
        self.defaults.inclusion_fee = uint;
        self
    }

    pub fn save(&self) -> Result<(), locator::Error> {
        let toml_string = toml::to_string(&self)?;
        let path = cli_config_file()?;
        // Depending on the platform, this function may fail if the full directory path does not exist
        let mut file = File::create(locator::ensure_directory(path)?)?;
        file.write_all(toml_string.as_bytes())?;

        Ok(())
    }
}
