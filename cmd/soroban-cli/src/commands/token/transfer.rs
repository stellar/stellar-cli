use std::ffi::OsString;

use clap::Parser;

use crate::{
    commands::{
        contract::invoke,
        global,
        token::args::{self, OutputFormat, TokenTarget},
    },
    config::{
        self, locator, network, sign_with, UnresolvedContract, UnresolvedMuxedAccount,
        UnresolvedScAddress,
    },
    output::Output,
};

#[derive(Debug, Parser, Clone)]
#[group(skip)]
pub struct Cmd {
    /// The token to transfer from: a contract id or alias, `native`, or a
    /// classic asset as `CODE:ISSUER`.
    #[arg(long = "id")]
    pub id: TokenTarget,

    /// Account to transfer tokens from. Signs and authorizes the transfer, so it
    /// must be an identity or secret key you control.
    #[arg(long)]
    pub from: UnresolvedMuxedAccount,

    /// Account or contract to transfer the tokens to. Accepts a `G…`/`M…`
    /// account, a `C…` contract address, or an alias.
    #[arg(long)]
    pub to: UnresolvedScAddress,

    /// Amount to transfer, in the token's smallest unit (stroops for a Stellar
    /// Asset Contract).
    #[arg(long, value_parser = parse_nonneg_i128)]
    pub amount: i128,

    /// Format of the output.
    #[arg(long, default_value = "text")]
    pub output: OutputFormat,

    #[command(flatten)]
    pub network: network::Args,

    #[command(flatten)]
    pub locator: locator::Args,

    #[command(flatten)]
    pub sign_with: sign_with::Args,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Config(#[from] config::Error),
    #[error(transparent)]
    Network(#[from] network::Error),
    #[error(transparent)]
    Args(#[from] args::Error),
    #[error(transparent)]
    ScAddress(#[from] config::sc_address::Error),
    #[error(transparent)]
    Invoke(#[from] invoke::Error),
    #[error(transparent)]
    Serde(#[from] serde_json::Error),

    #[error(
        "muxed (M…) source accounts are not yet supported for `token transfer`; \
         use the underlying G… account as `--from` instead"
    )]
    MuxedSourceNotSupported,
}

/// Parse `--amount` as a non-negative `i128`. A negative transfer amount is
/// always invalid, so reject it at the clap layer instead of letting it reach
/// the contract and fail as an opaque `HostError` deep in simulation.
fn parse_nonneg_i128(value: &str) -> Result<i128, String> {
    let amount: i128 = value
        .parse()
        .map_err(|_| format!("invalid amount: {value}"))?;
    if amount < 0 {
        return Err(format!("amount must not be negative: {value}"));
    }
    Ok(amount)
}

impl Error {
    /// Machine-readable discriminator for the JSON error envelope's `type` field.
    #[must_use]
    pub fn error_type(&self) -> &'static str {
        match self {
            Error::Config(_) => "config",
            Error::Network(_) => "network",
            Error::Args(e) => e.error_type(),
            Error::ScAddress(_) => "invalid_address",
            Error::Invoke(_) => "invoke",
            Error::Serde(_) => "internal",
            Error::MuxedSourceNotSupported => "unsupported",
        }
    }
}

/// The machine-readable receipt of a token transfer.
#[derive(Debug, serde::Serialize)]
struct Receipt {
    /// Hex-encoded hash of the submitted transaction.
    tx_hash: Option<String>,
    /// The decoded contract return value (`null` for SEP-41 `transfer`, which
    /// returns nothing).
    result: serde_json::Value,
}

impl Cmd {
    /// Assemble a full [`config::Args`] for the underlying invocation, using
    /// `--from` as the source account that signs and authorizes the transfer.
    /// Fees are left unset so the pipeline applies its default inclusion fee —
    /// this command intentionally exposes no fee or sequence knobs.
    fn config(&self) -> config::Args {
        config::Args {
            network: self.network.clone(),
            source_account: self.from.clone(),
            locator: self.locator.clone(),
            sign_with: self.sign_with.clone(),
            fee: None,
            inclusion_fee: None,
        }
    }

    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let output = Output::new(self.output.into(), global_args.quiet);
        // In JSON mode the underlying invoke pipeline's human-readable status
        // logging (which writes to stderr) would still fire; run it quietly so
        // machine consumers get clean output without needing `--quiet`.
        let quiet = global_args.quiet || output.is_json();
        let config = self.config();
        let network = config.get_network()?;

        let resolved = self
            .id
            .resolve(&config.locator, &network.network_passphrase)?;

        // SEP-41 `transfer(from, to, amount)`: `from` is the source account
        // (which also signs and authorizes), `to` is the destination.
        //
        // The invoke pipeline can't source a transaction from a muxed account
        // yet (see #2645), and a muxed strkey in the `transfer` arg is rejected
        // mid-simulation with an opaque host error; reject it up front with a
        // clear message instead.
        let source_account = config.source_account()?;
        if matches!(source_account, crate::xdr::MuxedAccount::MuxedEd25519(_)) {
            return Err(Error::MuxedSourceNotSupported);
        }
        let from = source_account.to_string();
        // `--to` may be an account (`G…`/`M…`), a contract (`C…`), or an alias;
        // resolve it to an `ScAddress` and hand the strkey to the `transfer`
        // arg, which accepts any of these destinations.
        let to = self
            .to
            .clone()
            .resolve(&config.locator, &network.network_passphrase, None)?
            .to_string();
        let amount = self.amount.to_string();

        let slop: Vec<OsString> = [
            "transfer", "--from", &from, "--to", &to, "--amount", &amount,
        ]
        .into_iter()
        .map(OsString::from)
        .collect();

        let invoke_cmd = invoke::Cmd {
            contract_id: UnresolvedContract::Resolved(resolved.contract_id),
            slop,
            config: config.clone(),
            // A transfer always intends to submit. Force `Send::Yes` so a token
            // whose `transfer` records no writes/events/auth can't be classified
            // read-only and silently exit 0 without ever moving funds.
            send: invoke::Send::Yes,
            ..Default::default()
        };

        let receipt = invoke_cmd
            .execute_with_receipt(&config, quiet, global_args.no_cache)
            .await
            .map_err(|e| {
                resolved
                    .not_deployed_error(&e)
                    .map_or(Error::Invoke(e), Error::Args)
            })?
            .into_result();

        // `transfer` always writes, so the invocation is submitted rather than
        // resolved as a build-only transaction; a missing receipt would mean
        // `--build-only`, which this command never sets.
        let Some(receipt) = receipt else {
            return Ok(());
        };

        let result = if receipt.output.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::from_str(&receipt.output)
                .unwrap_or(serde_json::Value::String(receipt.output.clone()))
        };

        // The pipeline already logs submission status and the explorer link to
        // stderr; echo the hash to stdout so readable output is scriptable too.
        if !output.is_json() {
            if let Some(tx_hash) = &receipt.tx_hash {
                println!("{tx_hash}");
            }
        }

        output.json_value(&Receipt {
            tx_hash: receipt.tx_hash,
            result,
        })?;

        Ok(())
    }
}
