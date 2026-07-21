use std::ffi::OsString;

use clap::Parser;

use crate::{
    commands::{
        contract::invoke,
        global,
        token::args::{self, OutputFormat, ResolvedToken, TokenTarget},
    },
    config::{self, locator, network, sign_with, UnresolvedContract, UnresolvedScAddress},
    fixed_point::FixedPoint,
    output::Output,
};

#[derive(Debug, Parser, Clone)]
#[group(skip)]
pub struct Cmd {
    /// The token to query: a contract id or alias, `native`, or a classic asset
    /// as `CODE:ISSUER`.
    #[arg(long = "id")]
    pub id: TokenTarget,

    /// Account or contract whose balance to read.
    #[arg(long)]
    pub account: UnresolvedScAddress,

    /// Format the balance as a decimal using the token's `decimals`, instead of
    /// the raw smallest unit (stroops for a Stellar Asset Contract).
    #[arg(long)]
    pub decimal: bool,

    /// Format of the output.
    #[arg(long, default_value = "text")]
    pub output: OutputFormat,

    #[command(flatten)]
    pub network: network::Args,

    #[command(flatten)]
    pub locator: locator::Args,
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

    #[error("could not parse {what} from the contract: {value:?}")]
    ParseResult { what: &'static str, value: String },
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
            Error::Serde(_) | Error::ParseResult { .. } => "internal",
        }
    }
}

/// The machine-readable result of a balance query.
#[derive(Debug, serde::Serialize)]
struct BalanceResult {
    /// The balance, in the requested representation: raw smallest units by
    /// default, or a decimal string when `--decimal` is set.
    balance: String,
    /// The token's `decimals`, present only when `--decimal` was requested.
    #[serde(skip_serializing_if = "Option::is_none")]
    decimals: Option<u32>,
}

impl Cmd {
    /// A read-only config: balance is resolved by simulation, so no source
    /// account, signing options, or fees are needed.
    fn config(&self) -> config::Args {
        config::Args {
            network: self.network.clone(),
            source_account: config::UnresolvedMuxedAccount::default(),
            locator: self.locator.clone(),
            sign_with: sign_with::Args::default(),
            fee: None,
            inclusion_fee: None,
        }
    }

    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let output = Output::new(self.output.into(), global_args.quiet);
        // Read-only calls still log through the invoke pipeline's Print; keep it
        // quiet in JSON mode so stdout stays pure JSON.
        let quiet = global_args.quiet || output.is_json();
        let config = self.config();
        let network = config.get_network()?;

        let resolved = self
            .id
            .resolve(&config.locator, &network.network_passphrase)?;
        let account = self
            .account
            .clone()
            .resolve(&config.locator, &network.network_passphrase, None)?
            .to_string();

        let raw: i128 = self
            .read_parsed(
                &config,
                quiet,
                global_args.no_cache,
                &resolved,
                vec![
                    OsString::from("balance"),
                    OsString::from("--id"),
                    OsString::from(&account),
                ],
                "balance",
            )
            .await?;

        let (balance, decimals) = if self.decimal {
            // Deliberately a second, separate simulation: `decimals` isn't
            // returned by `balance`, so `--decimal` costs one extra read-only
            // RPC round-trip on top of the balance query.
            let decimals: u32 = self
                .read_parsed(
                    &config,
                    quiet,
                    global_args.no_cache,
                    &resolved,
                    vec![OsString::from("decimals")],
                    "decimals",
                )
                .await?;
            (FixedPoint::new(raw, decimals).to_string(), Some(decimals))
        } else {
            (raw.to_string(), None)
        };

        output.readable(|_| println!("{balance}"));
        output.json_value(&BalanceResult { balance, decimals })?;

        Ok(())
    }

    /// Invoke a read-only token function and return its decoded output string.
    async fn read(
        &self,
        config: &config::Args,
        quiet: bool,
        no_cache: bool,
        resolved: &ResolvedToken,
        slop: Vec<OsString>,
    ) -> Result<String, Error> {
        let invoke_cmd = invoke::Cmd {
            contract_id: UnresolvedContract::Resolved(resolved.contract_id),
            slop,
            config: config.clone(),
            send: invoke::Send::No,
            ..Default::default()
        };

        let receipt = invoke_cmd
            .execute_with_receipt(config, quiet, no_cache)
            .await
            .map_err(|e| {
                resolved
                    .not_deployed_error(&e)
                    .map_or(Error::Invoke(e), Error::Args)
            })?
            .into_result();

        Ok(receipt.map(|r| r.output).unwrap_or_default())
    }

    /// Invoke a read-only token function, then parse its decoded output as `T`.
    /// `what` labels the value in a `ParseResult` error if parsing fails.
    async fn read_parsed<T: std::str::FromStr>(
        &self,
        config: &config::Args,
        quiet: bool,
        no_cache: bool,
        resolved: &ResolvedToken,
        slop: Vec<OsString>,
        what: &'static str,
    ) -> Result<T, Error> {
        let out = self.read(config, quiet, no_cache, resolved, slop).await?;
        // A 128-bit balance comes back JSON-encoded as a quoted string (it can't
        // fit a JSON number), while `decimals` (u32) comes back bare; strip any
        // surrounding quotes so both parse straight into `T`.
        out.trim()
            .trim_matches('"')
            .parse()
            .map_err(|_| Error::ParseResult { what, value: out })
    }
}
