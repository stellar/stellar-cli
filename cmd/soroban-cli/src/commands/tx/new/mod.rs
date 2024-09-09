use clap::Parser;

use super::global;

mod account_merge;
mod bump_sequence;
mod change_trust;
mod create_account;
mod manage_data;
mod payment;
mod set_options;
mod set_trustline_flags;

#[derive(Debug, Parser)]
pub enum Cmd {
    /// Transfers the XLM balance of an account to another account and removes the source account from the ledger
    /// Threshold: High
    AccountMerge(account_merge::Cmd),
    /// Bumps forward the sequence number of the source account to the given sequence number, invalidating any transaction with a smaller sequence number
    /// Threshold: Low
    BumpSequence(bump_sequence::Cmd),
    /// Creates, updates, or deletes a trustline
    /// Learn more about trustlines: [Trustlines Encyclopedia Entry](https://developers.stellar.org/docs/learn/encyclopedia/transactions-specialized/trustlines/)
    /// Threshold: Medium
    ChangeTrust(change_trust::Cmd),
    /// Creates and funds a new account with the specified starting balance
    /// Threshold: Medium
    CreateAccount(create_account::Cmd),
    /// Sets, modifies, or deletes a data entry (name/value pair) that is attached to an account
    /// Learn more about entries and subentries: [Accounts section](../stellar-data-structures/accounts.mdx#subentries)
    /// Threshold: Medium
    ManageData(manage_data::Cmd),
    /// Sends an amount in a specific asset to a destination account
    /// Threshold: Medium
    Payment(payment::Cmd),
    /// Set option for an account such as flags, inflation destination, signers, home domain, and master key weight
    /// Learn more about flags: [Flags Encyclopedia Entry](../../glossary.mdx#flags)  
    /// Learn more about the home domain: [Stellar Ecosystem Proposals SEP-0001](https://github.com/stellar/stellar-protocol/blob/master/ecosystem/sep-0001.md)  
    /// Learn more about signers operations and key weight: [Signature and Multisignature Encyclopedia Entry](../../encyclopedia/security/signatures-multisig.mdx)
    /// Threshold: High
    SetOptions(set_options::Cmd),
    /// Allows issuing account to configure authorization and trustline flags to an asset
    /// The Asset parameter is of the `TrustLineAsset` type. If you are modifying a trustline to a regular asset (i.e. one in a Code:Issuer format), this is equivalent to the Asset type.
    /// If you are modifying a trustline to a pool share, however, this is composed of the liquidity pool's unique ID.
    /// Learn more about flags: [Flags Glossary Entry](../../glossary.mdx#flags)
    /// Threshold: Low
    SetTrustlineFlags(set_trustline_flags::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    AccountMerge(#[from] account_merge::Error),
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
