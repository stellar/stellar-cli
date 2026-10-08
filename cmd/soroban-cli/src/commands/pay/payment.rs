//! The on-chain leg of an x402 payment.
//!
//! Drives a SEP-41 `transfer(from, to, amount)` through the `contract invoke`
//! pipeline (build → simulate → authorization-sign) to produce the signed,
//! unsubmitted transaction the facilitator settles, then packages it into the
//! x402 payment proof. Reads the asset's `decimals` through the same pipeline so
//! the spend cap can be enforced in display units.

use serde_json::{json, Value};

use crate::{
    commands::{contract::invoke, global},
    config::{self, UnresolvedMuxedAccount},
    utils::{contract_id_from_str, XDR_DEPTH_LIMIT},
    xdr::{Limits, MuxedAccount, WriteXdr},
};

/// Estimated ledger close time in seconds, used to convert the challenge's
/// `maxTimeoutSeconds` into an authorization-signature expiration ledger. Matches
/// the x402 reference client's default when a live estimate isn't available.
const ESTIMATED_LEDGER_CLOSE_SECONDS: u64 = 5;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Invoke(#[from] invoke::Error),

    #[error(transparent)]
    Config(#[from] config::Error),

    #[error(transparent)]
    Network(#[from] config::network::Error),

    #[error(transparent)]
    Rpc(#[from] soroban_rpc::Error),

    #[error(transparent)]
    Xdr(#[from] crate::xdr::Error),

    #[error("the asset at {asset} did not return a valid decimals value (`{value}`)")]
    InvalidDecimals { asset: String, value: String },

    #[error("the payment option's asset `{0}` is not a valid contract id")]
    InvalidAsset(String),

    #[error("muxed (M…) accounts are not yet supported as the paying account")]
    MuxedNotSupported,
}

impl Error {
    /// Machine-readable discriminator for the JSON error envelope's `type` field.
    #[must_use]
    pub fn error_type(&self) -> &'static str {
        match self {
            Error::Invoke(_) => "invoke",
            Error::Config(_) => "config",
            Error::Network(_) => "network",
            Error::Rpc(_) => "rpc",
            Error::Xdr(_) => "internal",
            Error::InvalidDecimals { .. } => "invalid_decimals",
            Error::InvalidAsset(_) => "invalid_asset",
            Error::MuxedNotSupported => "unsupported",
        }
    }
}

/// Number of ledgers to add to the current ledger so the authorization signature
/// stays valid for at least `max_timeout_seconds`.
fn expiry_ledgers(max_timeout_seconds: u64) -> u32 {
    // Round up so a short timeout still yields at least one ledger of validity.
    let ledgers = max_timeout_seconds
        .max(1)
        .div_ceil(ESTIMATED_LEDGER_CLOSE_SECONDS)
        .max(1);
    u32::try_from(ledgers).unwrap_or(u32::MAX)
}

/// Build the base `contract invoke` command for a SEP-41 call on `asset`,
/// supplying the arguments by position so the call works regardless of what the
/// deployed token names its parameters.
fn invoke_cmd(
    config: &config::Args,
    asset: stellar_strkey::Contract,
    function: &str,
    args: Vec<String>,
    send: invoke::Send,
) -> invoke::Cmd {
    invoke::Cmd {
        contract_id: config::UnresolvedContract::Resolved(asset),
        invocation: Some(invoke::PositionalInvocation {
            function: function.to_string(),
            args,
        }),
        config: config.clone(),
        send,
        ..Default::default()
    }
}

/// The paying account as an argument string that resolves both to an on-chain
/// address and to a signing key: the original alias/secret when given that way,
/// otherwise the resolved `G…` address (matched to a stored identity by public
/// key). Rejects muxed accounts, which can't yet source or sign.
fn paying_account_arg(config: &config::Args) -> Result<String, Error> {
    if matches!(config.source_account()?, MuxedAccount::MuxedEd25519(_)) {
        return Err(Error::MuxedNotSupported);
    }
    Ok(match &config.source_account {
        UnresolvedMuxedAccount::AliasOrSecret(s) => s.clone(),
        UnresolvedMuxedAccount::Resolved(_) => config.source_account()?.to_string(),
    })
}

/// Read the asset's `decimals` through a read-only invocation, for rendering the
/// spend cap in display units.
pub async fn read_decimals(
    config: &config::Args,
    asset: stellar_strkey::Contract,
    quiet: bool,
    no_cache: bool,
) -> Result<u32, Error> {
    let cmd = invoke_cmd(config, asset, "decimals", vec![], invoke::Send::No);
    let global_args = global::Args {
        locator: config.locator.clone(),
        filter_logs: Vec::default(),
        quiet,
        verbose: false,
        very_verbose: false,
        no_cache,
    };
    let output = cmd
        .invoke(&global_args)
        .await?
        .into_result()
        .unwrap_or_default();
    output
        .trim()
        .parse::<u32>()
        .map_err(|_| Error::InvalidDecimals {
            asset: format!("{asset}"),
            value: output,
        })
}

/// Build, simulate, and authorization-sign the `transfer` and package it into the
/// x402 payment proof. The chosen requirement (`accepted`) and `resource` are
/// echoed back verbatim as the server sent them.
#[allow(clippy::too_many_arguments)]
pub async fn build_proof(
    config: &config::Args,
    asset: stellar_strkey::Contract,
    pay_to: &str,
    amount: i128,
    max_timeout_seconds: u64,
    x402_version: u32,
    accepted: &Value,
    resource: Option<&Value>,
    quiet: bool,
    no_cache: bool,
) -> Result<Value, Error> {
    let network = config.get_network()?;
    let client = network.rpc_client()?;
    let latest_ledger = client.get_latest_ledger().await?.sequence;
    let expiration_ledger = latest_ledger.saturating_add(expiry_ledgers(max_timeout_seconds));

    let from = paying_account_arg(config)?;
    let cmd = invoke_cmd(
        config,
        asset,
        "transfer",
        vec![from, pay_to.to_string(), amount.to_string()],
        invoke::Send::Yes,
    );

    let tx = cmd
        .build_payment_authorization(config, expiration_ledger, quiet, no_cache)
        .await?;
    let transaction = tx.to_xdr_base64(Limits::depth(XDR_DEPTH_LIMIT))?;

    let mut payload = json!({
        "x402Version": x402_version,
        "payload": { "transaction": transaction },
        "accepted": accepted,
    });
    if let Some(resource) = resource {
        payload["resource"] = resource.clone();
    }
    Ok(payload)
}

/// Parse an `asset` contract id string (`C…`) from the challenge.
pub fn parse_asset(asset: &str) -> Result<stellar_strkey::Contract, Error> {
    contract_id_from_str(asset).map_err(|_| Error::InvalidAsset(asset.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expiry_ledgers_rounds_up_from_seconds() {
        // 60s / 5s per ledger = 12 ledgers.
        assert_eq!(expiry_ledgers(60), 12);
        // 61s rounds up to 13.
        assert_eq!(expiry_ledgers(61), 13);
        // A sub-ledger timeout still yields at least one ledger.
        assert_eq!(expiry_ledgers(1), 1);
        assert_eq!(expiry_ledgers(0), 1);
    }
}
