use clap::Parser;

use super::global;

mod account_merge;
mod begin_sponsoring_future_reserves;
mod bump_sequence;
mod change_trust;
mod create_account;
mod manage_data;
mod payment;
mod set_options;
mod set_trustline_flags;

#[derive(Debug, Parser)]
pub enum Cmd {
    /// Merge an account into another account
    AccountMerge(account_merge::Cmd),
    /// Allows an account to pay the base reserves for another account; sponsoring account establishes the is-sponsoring-future-reserves relationship
    /// There must also be an end sponsoring future reserves operation in the same transaction
    /// Learn more about sponsored reserves: [Sponsored Reserves Encyclopedia Entry](https://developers.stellar.org/docs/learn/encyclopedia/transactions-specialized/sponsored-reserves/)
    BeginSponsoringFutureReserves(begin_sponsoring_future_reserves::Cmd),
    /// Bump the sequence number of an account
    BumpSequence(bump_sequence::Cmd),
    /// Change trust for an asset
    ChangeTrust(change_trust::Cmd),
    /// Create a new account using another account
    CreateAccount(create_account::Cmd),
    /// Manage data on an account
    ManageData(manage_data::Cmd),
    /// Send a payment to an account
    Payment(payment::Cmd),
    /// Set options on an account
    SetOptions(set_options::Cmd),
    /// Set trustline flags on an account
    SetTrustlineFlags(set_trustline_flags::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    AccountMerge(#[from] account_merge::Error),
    #[error(transparent)]
    BeginSponsoringFutureReserves(#[from] begin_sponsoring_future_reserves::Error),
    #[error(transparent)]
    BumpSequence(#[from] bump_sequence::Error),
    #[error(transparent)]
    ChangeTrust(#[from] change_trust::Error),
    #[error(transparent)]
    CreateAccount(#[from] create_account::Error),
    #[error(transparent)]
    ManageData(#[from] manage_data::Error),
    #[error(transparent)]
    Payment(#[from] payment::Error),
    #[error(transparent)]
    SetOptions(#[from] set_options::Error),
    #[error(transparent)]
    SetTrustlineFlags(#[from] set_trustline_flags::Error),
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        match self {
            Cmd::AccountMerge(cmd) => cmd.run(global_args).await?,
            Cmd::BeginSponsoringFutureReserves(cmd) => cmd.run(global_args).await?,
            Cmd::BumpSequence(cmd) => cmd.run(global_args).await?,
            Cmd::ChangeTrust(cmd) => cmd.run(global_args).await?,
            Cmd::CreateAccount(cmd) => cmd.run(global_args).await?,
            Cmd::ManageData(cmd) => cmd.run(global_args).await?,
            Cmd::Payment(cmd) => cmd.run(global_args).await?,
            Cmd::SetOptions(cmd) => cmd.run(global_args).await?,
            Cmd::SetTrustlineFlags(cmd) => cmd.run(global_args).await?,
        };
        Ok(())
    }
}
