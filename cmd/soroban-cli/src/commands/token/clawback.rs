use clap::Parser;

use crate::{
    commands::{
        contract::invoke,
        global,
        token::args::{self, OutputFormat},
    },
    config::{
        self, locator, network, sign_with, token::UnresolvedToken, UnresolvedMuxedAccount,
        UnresolvedScAddress,
    },
    output::Output,
};

#[derive(Debug, Parser, Clone)]
#[group(skip)]
pub struct Cmd {
    /// The token to claw back: a contract id or alias, `native`, or a classic
    /// asset as `CODE:ISSUER`.
    #[arg(long = "id")]
    pub id: UnresolvedToken,

    /// The token's administrator. Signs and authorizes the clawback, so it must
    /// be an identity or secret key you control (the asset issuer for a Stellar
    /// Asset Contract).
    #[arg(long)]
    pub admin: UnresolvedMuxedAccount,

    /// Account or contract to claw the tokens back from. Accepts a `G…`/`M…`
    /// account, a `C…` contract address, or an alias.
    #[arg(long)]
    pub from: UnresolvedScAddress,

    /// Amount to claw back, in the token's smallest unit (stroops for a Stellar
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
    Token(#[from] config::token::Error),
    #[error(transparent)]
    ScAddress(#[from] config::sc_address::Error),
    #[error(transparent)]
    Invoke(#[from] invoke::Error),
    #[error(transparent)]
    Serde(#[from] serde_json::Error),

    #[error(
        "muxed (M…) source accounts are not yet supported for `token clawback`; \
         use the underlying G… account as `--admin` instead"
    )]
    MuxedSourceNotSupported,
}

/// Parse `--amount` as a non-negative `i128`. A negative clawback amount is
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
            Error::Token(e) => e.error_type(),
            Error::ScAddress(_) => "invalid_address",
            Error::Invoke(_) => "invoke",
            Error::Serde(_) => "internal",
            Error::MuxedSourceNotSupported => "unsupported",
        }
    }
}

/// The machine-readable receipt of a token clawback.
#[derive(Debug, serde::Serialize)]
struct Receipt {
    /// Hex-encoded hash of the submitted transaction.
    tx_hash: Option<String>,
    /// The decoded contract return value (`null` for the SAC `clawback`, which
    /// returns nothing).
    result: serde_json::Value,
}

impl Cmd {
    /// Assemble a full [`config::Args`] for the underlying invocation, using
    /// `--admin` as the source account that signs and authorizes the clawback.
    /// Fees are left unset so the pipeline applies its default inclusion fee —
    /// this command intentionally exposes no fee or sequence knobs.
    fn config(&self) -> config::Args {
        config::Args {
            network: self.network.clone(),
            source_account: self.admin.clone(),
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

        let token = self
            .id
            .resolve(&config.locator, &network.network_passphrase)?;

        // The admin only authorizes the clawback; it is not itself a `clawback`
        // argument. The invoke pipeline can't source a transaction from a muxed
        // account yet (see #2645), so reject a muxed admin up front with a clear
        // message.
        let source_account = config.source_account()?;
        if matches!(source_account, crate::xdr::MuxedAccount::MuxedEd25519(_)) {
            return Err(Error::MuxedSourceNotSupported);
        }
        // `--from` may be an account (`G…`/`M…`), a contract (`C…`), or an alias;
        // resolve it to an `ScAddress` and hand the strkey to the `clawback` arg,
        // which accepts any of these holders.
        let from = self
            .from
            .clone()
            .resolve(&config.locator, &network.network_passphrase, None)?
            .to_string();
        let amount = self.amount.to_string();

        // SAC `clawback(from, amount)` — supply the values in that order and let
        // the contract's parameters be matched by position. A clawback always
        // intends to submit, so force `Send::Yes`: a token whose `clawback`
        // records no writes/events/auth can't be classified read-only and
        // silently exit 0 without ever removing the balance.
        let receipt = args::invoke_by_position(
            &config,
            quiet,
            global_args.no_cache,
            &token,
            "clawback",
            vec![from, amount],
            invoke::Send::Yes,
        )
        .await
        .map_err(|e| args::not_deployed_error(&token, &e).map_or(Error::Invoke(e), Error::Args))?
        .into_result();

        // `clawback` always writes, so the invocation is submitted rather than
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
