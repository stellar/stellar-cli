use super::super::{global, help, xdr::tx_envelope_from_input};
use crate::xdr::{OperationBody, WriteXdr};

pub(crate) use super::super::new;

mod account_merge;
mod args;
mod bump_sequence;
mod change_trust;
mod claim_claimable_balance;
mod clawback_claimable_balance;
mod create_account;
mod create_claimable_balance;
mod create_passive_sell_offer;
mod manage_buy_offer;
mod manage_data;
mod manage_sell_offer;
mod path_payment_strict_receive;
mod path_payment_strict_send;
mod payment;
mod set_options;
mod set_trustline_flags;

#[derive(Debug, clap::Parser)]
#[allow(clippy::doc_markdown)]
pub enum Cmd {
    #[command(about = help::ACCOUNT_MERGE)]
    AccountMerge(account_merge::Cmd),
    #[command(about = help::BUMP_SEQUENCE)]
    BumpSequence(bump_sequence::Cmd),
    #[command(about = help::CHANGE_TRUST)]
    ChangeTrust(change_trust::Cmd),
    #[command(about = help::CLAIM_CLAIMABLE_BALANCE)]
    ClaimClaimableBalance(claim_claimable_balance::Cmd),
    #[command(about = help::CLAWBACK_CLAIMABLE_BALANCE)]
    ClawbackClaimableBalance(clawback_claimable_balance::Cmd),
    #[command(about = help::CREATE_ACCOUNT)]
    CreateAccount(create_account::Cmd),
    #[command(about = help::CREATE_CLAIMABLE_BALANCE)]
    CreateClaimableBalance(create_claimable_balance::Cmd),
    #[command(about = help::CREATE_PASSIVE_SELL_OFFER)]
    CreatePassiveSellOffer(create_passive_sell_offer::Cmd),
    #[command(about = help::MANAGE_BUY_OFFER)]
    ManageBuyOffer(manage_buy_offer::Cmd),
    #[command(about = help::MANAGE_DATA)]
    ManageData(manage_data::Cmd),
    #[command(about = help::MANAGE_SELL_OFFER)]
    ManageSellOffer(manage_sell_offer::Cmd),
    #[command(about = help::PATH_PAYMENT_STRICT_RECEIVE)]
    PathPaymentStrictReceive(path_payment_strict_receive::Cmd),
    #[command(about = help::PATH_PAYMENT_STRICT_SEND)]
    PathPaymentStrictSend(path_payment_strict_send::Cmd),
    #[command(about = help::PAYMENT)]
    Payment(payment::Cmd),
    #[command(about = help::SET_OPTIONS)]
    SetOptions(set_options::Cmd),
    #[command(about = help::SET_TRUSTLINE_FLAGS)]
    SetTrustlineFlags(set_trustline_flags::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    TxXdr(#[from] super::super::xdr::Error),
    #[error(transparent)]
    Xdr(#[from] crate::xdr::Error),
    #[error(transparent)]
    New(#[from] super::super::new::Error),
    #[error(transparent)]
    Tx(#[from] super::super::args::Error),
}

impl TryFrom<&Cmd> for OperationBody {
    type Error = super::super::new::Error;
    fn try_from(cmd: &Cmd) -> Result<Self, Self::Error> {
        Ok(match &cmd {
            Cmd::AccountMerge(account_merge::Cmd { op, .. }) => op.try_into()?,
            Cmd::BumpSequence(bump_sequence::Cmd { op, .. }) => op.into(),
            Cmd::ChangeTrust(change_trust::Cmd { op, .. }) => op.try_into()?,
            Cmd::ClaimClaimableBalance(claim_claimable_balance::Cmd { op, .. }) => op.try_into()?,
            Cmd::ClawbackClaimableBalance(clawback_claimable_balance::Cmd { op, .. }) => {
                op.try_into()?
            }
            Cmd::CreateAccount(create_account::Cmd { op, .. }) => op.try_into()?,
            Cmd::CreateClaimableBalance(create_claimable_balance::Cmd { op, .. }) => {
                op.try_into()?
            }
            Cmd::CreatePassiveSellOffer(create_passive_sell_offer::Cmd { op, .. }) => {
                op.try_into()?
            }
            Cmd::ManageBuyOffer(manage_buy_offer::Cmd { op, .. }) => op.try_into()?,
            Cmd::ManageData(manage_data::Cmd { op, .. }) => op.into(),
            Cmd::ManageSellOffer(manage_sell_offer::Cmd { op, .. }) => op.try_into()?,
            Cmd::PathPaymentStrictReceive(path_payment_strict_receive::Cmd { op, .. }) => {
                op.try_into()?
            }
            Cmd::PathPaymentStrictSend(path_payment_strict_send::Cmd { op, .. }) => {
                op.try_into()?
            }
            Cmd::Payment(payment::Cmd { op, .. }) => op.try_into()?,
            Cmd::SetOptions(set_options::Cmd { op, .. }) => op.try_into()?,
            Cmd::SetTrustlineFlags(set_trustline_flags::Cmd { op, .. }) => op.try_into()?,
        })
    }
}

impl Cmd {
    pub async fn run(&self, _: &global::Args) -> Result<(), Error> {
        let op = OperationBody::try_from(self)?;
        let res = match self {
            Cmd::AccountMerge(cmd) => cmd.op.tx.add_op(
                op,
                tx_envelope_from_input(&cmd.args.tx_xdr)?,
                cmd.args.source(),
            ),
            Cmd::BumpSequence(cmd) => cmd.op.tx.add_op(
                op,
                tx_envelope_from_input(&cmd.args.tx_xdr)?,
                cmd.args.source(),
            ),
            Cmd::ChangeTrust(cmd) => cmd.op.tx.add_op(
                op,
                tx_envelope_from_input(&cmd.args.tx_xdr)?,
                cmd.args.source(),
            ),
            Cmd::ClaimClaimableBalance(cmd) => cmd.op.tx.add_op(
                op,
                tx_envelope_from_input(&cmd.args.tx_xdr)?,
                cmd.args.source(),
            ),
            Cmd::ClawbackClaimableBalance(cmd) => cmd.op.tx.add_op(
                op,
                tx_envelope_from_input(&cmd.args.tx_xdr)?,
                cmd.args.source(),
            ),
            Cmd::CreateAccount(cmd) => cmd.op.tx.add_op(
                op,
                tx_envelope_from_input(&cmd.args.tx_xdr)?,
                cmd.args.source(),
            ),
            Cmd::CreateClaimableBalance(cmd) => cmd.op.tx.add_op(
                op,
                tx_envelope_from_input(&cmd.args.tx_xdr)?,
                cmd.args.source(),
            ),
            Cmd::CreatePassiveSellOffer(cmd) => cmd.op.tx.add_op(
                op,
                tx_envelope_from_input(&cmd.args.tx_xdr)?,
                cmd.args.source(),
            ),
            Cmd::ManageBuyOffer(cmd) => cmd.op.tx.add_op(
                op,
                tx_envelope_from_input(&cmd.args.tx_xdr)?,
                cmd.args.source(),
            ),
            Cmd::ManageData(cmd) => cmd.op.tx.add_op(
                op,
                tx_envelope_from_input(&cmd.args.tx_xdr)?,
                cmd.args.source(),
            ),
            Cmd::ManageSellOffer(cmd) => cmd.op.tx.add_op(
                op,
                tx_envelope_from_input(&cmd.args.tx_xdr)?,
                cmd.args.source(),
            ),
            Cmd::PathPaymentStrictReceive(cmd) => cmd.op.tx.add_op(
                op,
                tx_envelope_from_input(&cmd.args.tx_xdr)?,
                cmd.args.source(),
            ),
            Cmd::PathPaymentStrictSend(cmd) => cmd.op.tx.add_op(
                op,
                tx_envelope_from_input(&cmd.args.tx_xdr)?,
                cmd.args.source(),
            ),
            Cmd::Payment(cmd) => cmd.op.tx.add_op(
                op,
                tx_envelope_from_input(&cmd.args.tx_xdr)?,
                cmd.args.source(),
            ),
            Cmd::SetOptions(cmd) => cmd.op.tx.add_op(
                op,
                tx_envelope_from_input(&cmd.args.tx_xdr)?,
                cmd.args.source(),
            ),
            Cmd::SetTrustlineFlags(cmd) => cmd.op.tx.add_op(
                op,
                tx_envelope_from_input(&cmd.args.tx_xdr)?,
                cmd.args.source(),
            ),
        }
        .await?;
        println!("{}", res.to_xdr_base64(crate::xdr::Limits::none())?);
        Ok(())
    }
}
