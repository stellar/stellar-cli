use clap::Parser;
use std::fmt::Debug;

pub mod account;
pub mod account_data;
pub mod args;
pub mod claimable_balance;
pub mod config;
pub mod contract_code;
pub mod contract_data;
pub mod liquidity_pool;
pub mod offer;
pub mod trustline;

#[derive(Debug, Parser)]
pub enum Cmd {
    /// Fetch account entry by public key or alias.
    Account(account::Cmd),
    /// Fetch contract ledger entry by address or alias and storage key.
    ContractData(contract_data::Cmd),
    /// Fetch the current network config by `ConfigSettingId`.
    /// All config settings are returned if no id is provided.
    Config(config::Cmd),
    ///Fetch a claimable balance ledger entry by id
    ClaimableBalance(claimable_balance::Cmd),
    ///Fetch a liquidity pool ledger entry by id
    LiquidityPool(liquidity_pool::Cmd),
    /// Fetch a Contract's WASM bytecode by WASM hash
    ContractCode(contract_code::Cmd),
    /// Fetch a trustline by account and asset
    Trustline(trustline::Cmd),
    /// Fetch key-value data entries attached to an account (see manageDataOp)
    Data(account_data::Cmd),
    /// Fetch an offer by account and offer id
    Offer(offer::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Account(#[from] account::Error),
    #[error(transparent)]
    ContractData(#[from] contract_data::Error),
    #[error(transparent)]
    Config(#[from] config::Error),
    #[error(transparent)]
    ClaimableBalance(#[from] claimable_balance::Error),
    #[error(transparent)]
    LiquidityPool(#[from] liquidity_pool::Error),
    #[error(transparent)]
    Wasm(#[from] contract_code::Error),
    #[error(transparent)]
    Trustline(#[from] trustline::Error),
    #[error(transparent)]
    Data(#[from] account_data::Error),
    #[error(transparent)]
    Offer(#[from] offer::Error),
}

impl Cmd {
    pub async fn run(&self) -> Result<(), Error> {
        match self {
            Cmd::Account(cmd) => cmd.run().await?,
            Cmd::ContractData(cmd) => cmd.run().await?,
            Cmd::Config(cmd) => cmd.run().await?,
            Cmd::ClaimableBalance(cmd) => cmd.run().await?,
            Cmd::LiquidityPool(cmd) => cmd.run().await?,
            Cmd::ContractCode(cmd) => cmd.run().await?,
            Cmd::Trustline(cmd) => cmd.run().await?,
            Cmd::Data(cmd) => cmd.run().await?,
            Cmd::Offer(cmd) => cmd.run().await?,
        }
        Ok(())
    }
}
