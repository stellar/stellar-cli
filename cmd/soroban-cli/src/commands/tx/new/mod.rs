use clap::Parser;
use soroban_sdk::xdr::OperationBody;

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
#[allow(clippy::doc_markdown)]
pub enum Cmd {
    /// Transfers the XLM balance of an account to another account and removes the source account from the ledger
    AccountMerge(account_merge::Cmd),
    /// Bumps forward the sequence number of the source account to the given sequence number, invalidating any transaction with a smaller sequence number
    BumpSequence(bump_sequence::Cmd),
    /// Creates, updates, or deletes a trustline
    /// Learn more about trustlines
    /// https://developers.stellar.org/docs/learn/fundamentals/stellar-data-structures/accounts#trustlines
    ChangeTrust(change_trust::Cmd),
    /// Creates and funds a new account with the specified starting balance
    CreateAccount(create_account::Cmd),
    /// Sets, modifies, or deletes a data entry (name/value pair) that is attached to an account
    /// Learn more about entries and subentries:
    /// https://developers.stellar.org/docs/learn/fundamentals/stellar-data-structures/accounts#subentries
    ManageData(manage_data::Cmd),
    /// Sends an amount in a specific asset to a destination account
    Payment(payment::Cmd),
    /// Set option for an account such as flags, inflation destination, signers, home domain, and master key weight
    /// Learn more about flags:
    /// https://developers.stellar.org/docs/learn/glossary#flags
    /// Learn more about the home domain:
    /// https://github.com/stellar/stellar-protocol/blob/master/ecosystem/sep-0001.md
    /// Learn more about signers operations and key weight:
    /// https://developers.stellar.org/docs/learn/encyclopedia/security/signatures-multisig#multisig
    SetOptions(set_options::Cmd),
    /// Allows issuing account to configure authorization and trustline flags to an asset
    /// The Asset parameter is of the `TrustLineAsset` type. If you are modifying a trustline to a regular asset (i.e. one in a Code:Issuer format), this is equivalent to the Asset type.
    /// If you are modifying a trustline to a pool share, however, this is composed of the liquidity pool's unique ID.
    /// Learn more about flags:
    /// https://developers.stellar.org/docs/learn/glossary#flags
    SetTrustlineFlags(set_trustline_flags::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Tx(#[from] super::args::Error),
}

impl TryFrom<&Cmd> for OperationBody {
    type Error = super::args::Error;
    fn try_from(cmd: &Cmd) -> Result<Self, Self::Error> {
        Ok(match cmd {
            Cmd::AccountMerge(cmd) => cmd.try_into()?,
            Cmd::BumpSequence(cmd) => cmd.into(),
            Cmd::ChangeTrust(cmd) => cmd.into(),
            Cmd::CreateAccount(cmd) => cmd.try_into()?,
            Cmd::ManageData(cmd) => cmd.into(),
            Cmd::Payment(cmd) => cmd.try_into()?,
            Cmd::SetOptions(cmd) => cmd.try_into()?,
            Cmd::SetTrustlineFlags(cmd) => cmd.try_into()?,
        })
    }
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let op = OperationBody::try_from(self)?;
        match self {
            Cmd::AccountMerge(cmd) => cmd.tx.handle_and_print(op, global_args).await,
            Cmd::BumpSequence(cmd) => cmd.tx.handle_and_print(op, global_args).await,
            Cmd::ChangeTrust(cmd) => cmd.tx.handle_and_print(op, global_args).await,
            Cmd::CreateAccount(cmd) => cmd.tx.handle_and_print(op, global_args).await,
            Cmd::ManageData(cmd) => cmd.tx.handle_and_print(op, global_args).await,
            Cmd::Payment(cmd) => cmd.tx.handle_and_print(op, global_args).await,
            Cmd::SetOptions(cmd) => cmd.tx.handle_and_print(op, global_args).await,
            Cmd::SetTrustlineFlags(cmd) => cmd.tx.handle_and_print(op, global_args).await,
        }?;
        Ok(())
    }
}
