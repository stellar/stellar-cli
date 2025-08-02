use clap::Parser;
use std::fmt::Debug;

pub mod account;
pub mod args;
pub mod claimable_balance;
pub mod config;
pub mod contract;
pub mod liquidity_pool;
pub mod wasm;

#[derive(Debug, Parser)]
pub enum Cmd {
    /// Fetch account entry by public key or alias.
    /// Additional account-related keys are available with optional flags.
    Account(account::Cmd),
    /// Fetch contract ledger entry by address or alias and storage key.
    Contract(contract::Cmd),
    /// Fetch the current network config by `ConfigSettingId`.
    /// All config settings are returned if no id is provided.
    Config(config::Cmd),
    ///Fetch a claimable balance ledger entry by id
    ClaimableBalance(claimable_balance::Cmd),
    ///Fetch a liquidity pool ledger entry by id
    LiquidityPool(liquidity_pool::Cmd),
    /// Fetch WASM bytecode by hash
    Wasm(wasm::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Account(#[from] account::Error),
    #[error(transparent)]
    Contract(#[from] contract::Error),
    #[error(transparent)]
    Config(#[from] config::Error),
    #[error(transparent)]
    ClaimableBalance(#[from] claimable_balance::Error),
    #[error(transparent)]
    LiquidityPool(#[from] liquidity_pool::Error),
    #[error(transparent)]
    Wasm(#[from] wasm::Error),
}

impl Cmd {
    pub async fn run(&self) -> Result<(), Error> {
        match self {
            Cmd::Account(cmd) => cmd.run().await?,
            Cmd::Contract(cmd) => cmd.run().await?,
            Cmd::Config(cmd) => cmd.run().await?,
            Cmd::ClaimableBalance(cmd) => cmd.run().await?,
            Cmd::LiquidityPool(cmd) => cmd.run().await?,
            Cmd::Wasm(cmd) => cmd.run().await?,
        }
        Ok(())
    }
}
