use clap::Parser;

use crate::{
    commands::{
        contract::invoke,
        global,
        token::args::{self, OutputFormat},
    },
    config::{self, locator, network, sign_with, token::UnresolvedToken},
    output::Output,
};

#[derive(Debug, Parser, Clone)]
#[group(skip)]
pub struct Cmd {
    /// The token to query: a contract id or alias, `native`, or a classic asset
    /// as `CODE:ISSUER`.
    #[arg(long = "id")]
    pub id: UnresolvedToken,

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
    Token(#[from] config::token::Error),
    #[error(transparent)]
    Invoke(#[from] invoke::Error),
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
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
            Error::Invoke(_) => "invoke",
            Error::Serde(_) => "internal",
        }
    }
}

/// The machine-readable result of a `symbol` query.
#[derive(Debug, serde::Serialize)]
struct SymbolResult {
    /// The token's SEP-41 `symbol` metadata.
    symbol: String,
}

impl Cmd {
    /// A read-only config: `symbol` is resolved by simulation, so no source
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

        let token = self
            .id
            .resolve(&config.locator, &network.network_passphrase)?;

        // SEP-41 `symbol()` takes no arguments and returns a `String`.
        let receipt = args::invoke_by_position(
            &config,
            quiet,
            global_args.no_cache,
            &token,
            "symbol",
            vec![],
            invoke::Send::No,
        )
        .await
        .map_err(|e| args::not_deployed_error(&token, &e).map_or(Error::Invoke(e), Error::Args))?
        .into_result();

        // The invoke pipeline renders the `String` return value as JSON, so
        // decode it back through serde rather than trimming quotes by hand —
        // that recovers a symbol containing quotes, backslashes, or control
        // characters intact instead of leaking the escape sequences. `symbol()`
        // always returns a value on a successful read, so a missing receipt
        // (build-only, which reads never set) decodes as an empty symbol.
        let symbol = match receipt {
            Some(r) => serde_json::from_str::<String>(r.output.trim())?,
            None => String::new(),
        };

        // The symbol is contract-controlled data, so escape any ANSI/control
        // sequences before writing it to the terminal — matching how other
        // contract-derived output is rendered (see `log::auth`/`log::event`).
        // JSON output keeps the exact value: serde re-escapes it safely.
        output.readable(|_| println!("{}", soroban_spec_tools::sanitize(&symbol)));
        output.json_value(&SymbolResult { symbol })?;

        Ok(())
    }
}
