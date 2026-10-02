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
    /// The token to burn from: a contract id or alias, `native`, or a classic
    /// asset as `CODE:ISSUER`.
    #[arg(long = "id")]
    pub id: UnresolvedToken,

    /// Spender drawing on its allowance. Signs and authorizes the burn, so it
    /// must be an identity or secret key you control.
    #[arg(long)]
    pub spender: UnresolvedMuxedAccount,

    /// Owner whose tokens are destroyed. Must have granted `--spender` an
    /// allowance. Accepts a `G…`/`M…` account, a `C…` contract address, or an
    /// alias.
    #[arg(long)]
    pub from: UnresolvedScAddress,

    /// Amount to burn, in the token's smallest unit (stroops for a Stellar Asset
    /// Contract).
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

    #[error("muxed (M…) accounts are not yet supported")]
    MuxedNotSupported,
}

/// Parse `--amount` as a non-negative `i128`. A negative burn amount is always
/// invalid, so reject it at the clap layer instead of letting it reach the
/// contract and fail as an opaque `HostError` deep in simulation.
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
            Error::MuxedNotSupported => "unsupported",
        }
    }
}

/// The machine-readable result of a delegated token burn.
#[derive(Debug, serde::Serialize)]
struct BurnFromResult {
    /// Hex-encoded hash of the submitted transaction.
    tx_hash: Option<String>,
    /// The decoded contract return value (`null` for SEP-41 `burn_from`, which
    /// returns nothing).
    result: serde_json::Value,
}

impl Cmd {
    /// Assemble a full [`config::Args`] for the underlying invocation, using
    /// `--spender` as the source account that signs and authorizes the burn.
    /// Fees are left unset so the pipeline applies its default inclusion fee —
    /// this command intentionally exposes no fee or sequence knobs.
    fn config(&self) -> config::Args {
        config::Args {
            network: self.network.clone(),
            source_account: self.spender.clone(),
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

        // SEP-41 `burn_from(spender, from, amount)`: `spender` is the source
        // account (which also signs and authorizes) drawing on its allowance,
        // `from` is the owner whose funds are destroyed.
        //
        // The invoke pipeline can't source a transaction from a muxed account
        // yet (see #2645), and a muxed strkey in the `spender` arg is rejected
        // mid-simulation with an opaque host error; reject it up front with a
        // clear message instead.
        let source_account = config.source_account()?;
        if matches!(source_account, crate::xdr::MuxedAccount::MuxedEd25519(_)) {
            return Err(Error::MuxedNotSupported);
        }
        let spender = source_account.to_string();
        // `--from` may be an account (`G…`), a contract (`C…`), or an alias;
        // resolve it to an `ScAddress` and hand the strkey to the `burn_from`
        // args, which accept any of these.
        //
        // The host rejects a muxed (`M…`) owner mid-simulation with an opaque
        // error, so reject one up front with a clear message — whether supplied
        // as a direct `M…` strkey or an alias resolving to a muxed key.
        if self
            .from
            .is_muxed(&config.locator, &network.network_passphrase)
        {
            return Err(Error::MuxedNotSupported);
        }
        let from = self
            .from
            .clone()
            .resolve(&config.locator, &network.network_passphrase, None)?
            .to_string();
        let amount = self.amount.to_string();

        // SEP-41 `burn_from(spender, from, amount)` — supply the values in that
        // order and let the contract's parameters be matched by position, so a
        // token that names them anything still works. A burn always intends to
        // submit, so force `Send::Yes`: a token whose `burn_from` records no
        // writes/events/auth can't be classified read-only and silently exit 0
        // without ever destroying funds.
        let invoke_result = args::invoke_by_position(
            &config,
            quiet,
            global_args.no_cache,
            &token,
            "burn_from",
            vec![spender, from, amount],
            invoke::Send::Yes,
        )
        .await
        .map_err(|e| args::not_deployed_error(&token, &e).map_or(Error::Invoke(e), Error::Args))?
        .into_result();

        // `burn_from` always writes, so the invocation is submitted rather than
        // resolved as a build-only transaction; a missing result would mean
        // `--build-only`, which this command never sets.
        let Some(invoke_result) = invoke_result else {
            return Ok(());
        };

        let result = if invoke_result.output.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::from_str(&invoke_result.output)
                .unwrap_or(serde_json::Value::String(invoke_result.output.clone()))
        };

        // The pipeline already logs submission status and the explorer link to
        // stderr; echo the hash to stdout so readable output is scriptable too.
        if !output.is_json() {
            if let Some(tx_hash) = &invoke_result.tx_hash {
                println!("{tx_hash}");
            }
        }

        output.json_value(&BurnFromResult {
            tx_hash: invoke_result.tx_hash,
            result,
        })?;

        Ok(())
    }
}
