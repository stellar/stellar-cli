//! The on-chain leg of an x402 payment.
//!
//! Drives a SEP-41 `transfer(from, to, amount)` through the `contract invoke`
//! pipeline (build → simulate → authorization-sign) to produce the signed,
//! unsubmitted transaction the facilitator settles, then packages it into the
//! x402 payment proof. Reads the asset's `decimals` through the same pipeline so
//! the spend cap can be enforced in display units.

use serde_json::{json, Value};

use crate::{
    commands::{contract::invoke, global, token::args::format_decimal},
    config::{self, UnresolvedMuxedAccount},
    utils::{contract_id_from_str, XDR_DEPTH_LIMIT},
    xdr::{
        Limits, MuxedAccount, Preconditions, TimeBounds, TimePoint, Transaction,
        TransactionEnvelope, TransactionV1Envelope, VecM, WriteXdr,
    },
};

/// Estimated ledger close time in seconds, used to convert the challenge's
/// `maxTimeoutSeconds` into an authorization-signature expiration ledger. Matches
/// the x402 reference client's default when a live estimate isn't available.
const ESTIMATED_LEDGER_CLOSE_SECONDS: u64 = 5;

/// Stellar Asset Contract error raised when a transfer would leave a balance
/// outside its allowed range (e.g. below zero).
const SAC_BALANCE_OUT_OF_RANGE: u32 = 10;
/// Stellar Asset Contract error raised when an account has no trustline for the
/// asset.
const SAC_TRUSTLINE_MISSING: u32 = 13;

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

    #[error(
        "`{account}` has no trustline for `{asset}`; add one with \
         `stellar tx new change-trust --source {account} --line {asset}`"
    )]
    PayerMissingTrustline { account: String, asset: String },

    #[error("`{account}` has {balance} `{asset}`, but the payment requires {required}")]
    InsufficientBalance {
        account: String,
        asset: String,
        balance: String,
        required: String,
    },

    #[error(
        "the server's pay-to account `{account}` has no trustline for `{asset}`, \
         so it can't receive the payment"
    )]
    RecipientMissingTrustline { account: String, asset: String },
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
            Error::PayerMissingTrustline { .. } => "payer_missing_trustline",
            Error::InsufficientBalance { .. } => "insufficient_balance",
            Error::RecipientMissingTrustline { .. } => "recipient_missing_trustline",
        }
    }
}

/// The payer's balance of the asset, as probed after a failed transfer.
#[derive(Debug, Clone, Copy)]
enum PayerBalance {
    MissingTrustline,
    Amount(i128),
    Unknown,
}

/// A transfer failure that can be attributed to a specific account.
#[derive(Debug, PartialEq, Eq)]
enum TransferFailure {
    PayerMissingTrustline,
    InsufficientBalance(i128),
    RecipientMissingTrustline,
}

/// The Stellar Asset Contract error code carried by a failed simulation, if it's
/// one that points at a trustline or balance problem.
fn sac_error_code(message: &str) -> Option<u32> {
    [SAC_BALANCE_OUT_OF_RANGE, SAC_TRUSTLINE_MISSING]
        .into_iter()
        .find(|code| message.contains(&format!("Error(Contract, #{code})")))
}

/// Attribute a failed transfer (SAC error `code`) using the payer's probed
/// balance. `None` when the cause can't be pinned on an account.
fn classify_transfer_failure(
    code: u32,
    payer: PayerBalance,
    amount: i128,
) -> Option<TransferFailure> {
    match payer {
        PayerBalance::MissingTrustline => Some(TransferFailure::PayerMissingTrustline),
        PayerBalance::Amount(balance) if balance < amount => {
            Some(TransferFailure::InsufficientBalance(balance))
        }
        PayerBalance::Amount(_) if code == SAC_TRUSTLINE_MISSING => {
            Some(TransferFailure::RecipientMissingTrustline)
        }
        _ => None,
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

/// Call a read-only token `function` and return its output with any JSON string
/// quoting removed.
async fn read(
    config: &config::Args,
    asset: stellar_strkey::Contract,
    function: &str,
    args: Vec<String>,
    quiet: bool,
    no_cache: bool,
) -> Result<String, invoke::Error> {
    let cmd = invoke_cmd(config, asset, function, args, invoke::Send::No);
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
    Ok(output.trim().trim_matches('"').to_string())
}

/// Read the asset's `decimals` through a read-only invocation, for rendering the
/// spend cap in display units.
pub async fn read_decimals(
    config: &config::Args,
    asset: stellar_strkey::Contract,
    quiet: bool,
    no_cache: bool,
) -> Result<u32, Error> {
    let output = read(config, asset, "decimals", vec![], quiet, no_cache).await?;
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
    let payer = config.source_account()?.to_string();
    let cmd = invoke_cmd(
        config,
        asset,
        "transfer",
        vec![from, pay_to.to_string(), amount.to_string()],
        invoke::Send::Yes,
    );

    let tx = match cmd
        .build_payment_authorization(config, expiration_ledger, quiet, no_cache)
        .await
    {
        Ok(tx) => tx,
        Err(err) => {
            return Err(explain_transfer_failure(
                config, asset, &payer, pay_to, amount, err, no_cache,
            )
            .await)
        }
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let transaction = encode_transaction(with_time_bounds(tx, now, max_timeout_seconds))?;

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

/// Turn a failed transfer simulation into an error naming the account at fault
/// when it's a trustline or balance problem, by probing the payer's balance.
/// Anything else is returned unchanged.
async fn explain_transfer_failure(
    config: &config::Args,
    asset: stellar_strkey::Contract,
    payer: &str,
    pay_to: &str,
    amount: i128,
    err: invoke::Error,
    no_cache: bool,
) -> Error {
    let Some(code) = sac_error_code(&err.to_string()) else {
        return err.into();
    };
    let balance = match read(
        config,
        asset,
        "balance",
        vec![payer.to_string()],
        true,
        no_cache,
    )
    .await
    {
        Ok(output) => output
            .parse()
            .map_or(PayerBalance::Unknown, PayerBalance::Amount),
        Err(e) if sac_error_code(&e.to_string()) == Some(SAC_TRUSTLINE_MISSING) => {
            PayerBalance::MissingTrustline
        }
        Err(_) => PayerBalance::Unknown,
    };
    let Some(failure) = classify_transfer_failure(code, balance, amount) else {
        return err.into();
    };
    let asset_name = read(config, asset, "name", vec![], true, no_cache)
        .await
        .unwrap_or_else(|_| format!("{asset}"));
    match failure {
        TransferFailure::PayerMissingTrustline => Error::PayerMissingTrustline {
            account: payer.to_string(),
            asset: asset_name,
        },
        TransferFailure::RecipientMissingTrustline => Error::RecipientMissingTrustline {
            account: pay_to.to_string(),
            asset: asset_name,
        },
        TransferFailure::InsufficientBalance(balance) => {
            let decimals = read_decimals(config, asset, true, no_cache).await.ok();
            Error::InsufficientBalance {
                account: payer.to_string(),
                asset: asset_name,
                balance: display_amount(balance, decimals),
                required: display_amount(amount, decimals),
            }
        }
    }
}

/// Render a raw token amount in display units, or as the raw integer when the
/// asset's decimals are unknown.
fn display_amount(value: i128, decimals: Option<u32>) -> String {
    decimals
        .and_then(|d| format_decimal(value, d).ok())
        .unwrap_or_else(|| value.to_string())
}

/// Bound the transaction to expire `max_timeout_seconds` after `now`.
fn with_time_bounds(tx: Transaction, now: u64, max_timeout_seconds: u64) -> Transaction {
    Transaction {
        cond: Preconditions::Time(TimeBounds {
            min_time: TimePoint(0),
            max_time: TimePoint(now.saturating_add(max_timeout_seconds)),
        }),
        ..tx
    }
}

/// Encode the auth-signed transaction as the base64 XDR of an unsigned v1
/// `TransactionEnvelope`, the form the facilitator parses.
fn encode_transaction(tx: Transaction) -> Result<String, Error> {
    let envelope = TransactionEnvelope::Tx(TransactionV1Envelope {
        tx,
        signatures: VecM::default(),
    });
    Ok(envelope.to_xdr_base64(Limits::depth(XDR_DEPTH_LIMIT))?)
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

    const MISSING_TRUSTLINE: &str = "transaction simulation failed: HostError: \
        Error(Contract, #13)\n\nEvent log (newest first):\n   0: [Diagnostic Event] ...";

    #[test]
    fn sac_error_code_detects_trustline_and_balance_errors() {
        assert_eq!(
            sac_error_code(MISSING_TRUSTLINE),
            Some(SAC_TRUSTLINE_MISSING)
        );
        assert_eq!(
            sac_error_code("HostError: Error(Contract, #10)"),
            Some(SAC_BALANCE_OUT_OF_RANGE)
        );
        assert_eq!(sac_error_code("HostError: Error(Contract, #1)"), None);
        assert_eq!(sac_error_code("HostError: Error(Contract, #130)"), None);
        assert_eq!(sac_error_code("could not reach the RPC server"), None);
    }

    #[test]
    fn payer_without_trustline_is_blamed_first() {
        for code in [SAC_TRUSTLINE_MISSING, SAC_BALANCE_OUT_OF_RANGE] {
            assert_eq!(
                classify_transfer_failure(code, PayerBalance::MissingTrustline, 100),
                Some(TransferFailure::PayerMissingTrustline)
            );
        }
    }

    #[test]
    fn payer_short_of_funds_is_insufficient_balance() {
        assert_eq!(
            classify_transfer_failure(SAC_BALANCE_OUT_OF_RANGE, PayerBalance::Amount(40), 100),
            Some(TransferFailure::InsufficientBalance(40))
        );
    }

    #[test]
    fn funded_payer_with_missing_trustline_blames_recipient() {
        assert_eq!(
            classify_transfer_failure(SAC_TRUSTLINE_MISSING, PayerBalance::Amount(100), 100),
            Some(TransferFailure::RecipientMissingTrustline)
        );
    }

    #[test]
    fn unattributable_failures_are_left_alone() {
        assert_eq!(
            classify_transfer_failure(SAC_BALANCE_OUT_OF_RANGE, PayerBalance::Amount(100), 100),
            None
        );
        assert_eq!(
            classify_transfer_failure(SAC_TRUSTLINE_MISSING, PayerBalance::Unknown, 100),
            None
        );
    }

    #[test]
    fn encoded_transaction_is_an_unsigned_v1_envelope() {
        use crate::xdr::{Memo, Preconditions, ReadXdr, SequenceNumber, TransactionExt, Uint256};

        let tx = Transaction {
            source_account: MuxedAccount::Ed25519(Uint256([0; 32])),
            fee: 100,
            seq_num: SequenceNumber(1),
            cond: Preconditions::None,
            memo: Memo::None,
            operations: VecM::default(),
            ext: TransactionExt::V0,
        };
        let encoded = encode_transaction(tx.clone()).unwrap();
        let envelope =
            TransactionEnvelope::from_xdr_base64(&encoded, Limits::depth(XDR_DEPTH_LIMIT)).unwrap();
        let TransactionEnvelope::Tx(v1) = envelope else {
            panic!("expected a v1 envelope");
        };
        assert_eq!(v1.tx, tx);
        assert!(v1.signatures.is_empty());
    }

    #[test]
    fn transaction_is_time_bounded_by_the_challenge_timeout() {
        use crate::xdr::{Memo, SequenceNumber, TransactionExt, Uint256};

        let tx = Transaction {
            source_account: MuxedAccount::Ed25519(Uint256([0; 32])),
            fee: 100,
            seq_num: SequenceNumber(1),
            cond: Preconditions::None,
            memo: Memo::None,
            operations: VecM::default(),
            ext: TransactionExt::V0,
        };
        let bounded = with_time_bounds(tx, 1_000, 300);
        assert_eq!(
            bounded.cond,
            Preconditions::Time(TimeBounds {
                min_time: TimePoint(0),
                max_time: TimePoint(1_300),
            })
        );
    }
}
