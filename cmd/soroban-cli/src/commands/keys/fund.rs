use clap::command;
use rand::{thread_rng, Rng};
use rust_decimal::Decimal;
use soroban_sdk::xdr;
use stellar_strkey::ed25519::{PrivateKey, PublicKey};

use crate::commands::config::secret::Secret;
use crate::commands::global;
use crate::commands::network::LOCAL_NETWORK_PASSPHRASE;
use crate::utils::contract_id_hash_from_asset;
use crate::utils::parsing::{self, parse_asset};
use crate::{commands, rpc, CommandParser};
use crate::{commands::network, utils::get_account_details};

use super::super::config::secret;
use super::address;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Address(#[from] address::Error),
    #[error(transparent)]
    Network(#[from] network::Error),
    #[error(transparent)]
    Secret(#[from] secret::Error),
    #[error(transparent)]
    Parsing(#[from] parsing::Error),
    #[error(transparent)]
    Rpc(#[from] rpc::Error),
    #[error(transparent)]
    Xdr(#[from] xdr::Error),
    #[error(transparent)]
    Clap(#[from] clap::Error),
    #[error("RPC URL is missing in the network configuration")]
    MissingRpcUrl,
    #[error("Network passphrase is missing in the network configuration")]
    MissingNetworkPassphrase,
    #[error("Asset contract could not be deployed")]
    AssetContractError,
    #[error("Failed to transfer funds: {0}")]
    FundTransferError(String),
    #[error("Problem deploying asset contract: {0}")]
    AssetDeploymentError(String),
}

#[derive(Debug, clap::Parser, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub network: network::Args,
    /// Address to fund
    #[command(flatten)]
    pub address: address::Cmd,
}

const STROOPS_PER_XLM: i64 = 10_000_000;
const DEFAULT_FRIENDBOT_AMOUNT: i64 = 10_000 * STROOPS_PER_XLM;

fn convert_xlm_rounded(balance: i64) -> Decimal {
    let xlm_balance = Decimal::new(balance, 0) / Decimal::new(STROOPS_PER_XLM, 0);
    xlm_balance.round_dp(2)
}

impl Cmd {
    pub async fn check_balance(&self) -> Result<i64, Error> {
        let rpc_url = self.network.rpc_url.as_ref().ok_or(Error::MissingRpcUrl)?;
        let network_passphrase = self
            .network
            .network_passphrase
            .as_ref()
            .ok_or(Error::MissingNetworkPassphrase)?;
        let client = rpc::Client::new(rpc_url)?;
        let key = self.address.private_key()?;
        let account_details =
            get_account_details(false, &client.clone(), network_passphrase, &key).await?;
        Ok(account_details.balance)
    }

    pub async fn create_temp_account(&self) -> Result<(PublicKey, PrivateKey), Error> {
        let random_seed: String = thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(16)
            .map(char::from)
            .collect();
        let secret = Secret::from_seed(Some(&random_seed))?;
        let addr = secret.public_key(self.address.hd_path)?;
        let private_key = secret.private_key(self.address.hd_path)?;
        let network = self.network.get(&self.address.locator)?;
        network
            .fund_address(&addr)
            .await
            .map_err(|e| {
                tracing::warn!("fund_address failed: {e}");
            })
            .unwrap_or_default();
        Ok((addr, private_key))
    }

    pub async fn get_asset_contract_id(&self) -> Result<stellar_strkey::Contract, Error> {
        // if network is local, deploy the Stellar Asset contract and retrieve ID
        let network = self.network.get(&self.address.locator)?;
        let asset = parse_asset("native")?;
        let rpc_url = self.network.rpc_url.as_ref().ok_or(Error::MissingRpcUrl)?;
        if network.network_passphrase == LOCAL_NETWORK_PASSPHRASE {
            let cmd = commands::contract::deploy::asset::Cmd::parse_arg_vec(&[
                "--asset",
                "native",
                "--source-account",
                &self.address.name,
                "--rpc-url",
                rpc_url,
                "--network-passphrase",
                &network.network_passphrase,
            ])?;
            match cmd.run().await {
                Ok(()) => {
                    let contract_id =
                        contract_id_hash_from_asset(&asset, &network.network_passphrase)?;
                    Ok(stellar_strkey::Contract(contract_id.0))
                }
                Err(err) => {
                    if err.to_string().contains("ExistingValue") {
                        let contract_id =
                            contract_id_hash_from_asset(&asset, &network.network_passphrase)?;
                        Ok(stellar_strkey::Contract(contract_id.0))
                    } else {
                        Err(Error::AssetDeploymentError(err.to_string()))
                    }
                }
            }
        } else {
            let contract_id = contract_id_hash_from_asset(&asset, &network.network_passphrase)?;
            Ok(stellar_strkey::Contract(contract_id.0))
        }
    }

    pub async fn add_funds(&self, amount: i64) -> Result<(), Error> {
        let id = self.get_asset_contract_id().await?;
        let addr = self.address.public_key()?;
        let rpc_url = self.network.rpc_url.as_ref().ok_or(Error::MissingRpcUrl)?;
        let network_passphrase = self
            .network
            .network_passphrase
            .as_ref()
            .ok_or(Error::MissingNetworkPassphrase)?;
        let (from_id, temp_secret) = self.create_temp_account().await?;
        let to_id = format!("{addr}");
        let cmd = commands::contract::invoke::Cmd::parse_arg_vec(&[
            "--id",
            &id.to_string(),
            "--source-account",
            &temp_secret.to_string(),
            "--rpc-url",
            rpc_url,
            "--network-passphrase",
            network_passphrase,
            "--",
            "transfer",
            "--to",
            &to_id,
            "--from",
            &from_id.to_string(),
            "--amount",
            &amount.to_string(),
        ])?;
        cmd.run(&global::Args {
            locator: self.address.locator.clone(),
            ..Default::default()
        })
        .await
        .map_err(|e| Error::FundTransferError(e.to_string()))
    }

    pub async fn run(&self) -> Result<(), Error> {
        let addr = self.address.public_key()?;
        let balance = self.check_balance().await.unwrap_or_else(|err| {
            eprintln!("Failed to check balance: {err}");
            0
        });
        let rounded_xlm = convert_xlm_rounded(balance);
        if balance >= DEFAULT_FRIENDBOT_AMOUNT {
            println!(
                "Current {} balance: {} XLM. Nothing to do.",
                self.address.name, rounded_xlm,
            );
        } else if balance == 0 {
            self.network
                .get(&self.address.locator)?
                .fund_address(&addr)
                .await?;
        } else {
            println!(
                "Current {} balance: {} XLM. Topping off...",
                self.address.name, rounded_xlm,
            );
            self.add_funds(DEFAULT_FRIENDBOT_AMOUNT - balance).await?;
            let balance = match self.check_balance().await {
                Ok(balance) => balance,
                Err(err) => {
                    eprintln!("Failed to check balance: {err}");
                    0
                }
            };
            println!(
                "New {} balance: {} XLM.",
                self.address.name,
                convert_xlm_rounded(balance),
            );
        }
        Ok(())
    }
}
