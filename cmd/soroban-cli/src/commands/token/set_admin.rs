use clap::Parser;

use crate::{
    commands::{
        contract::invoke,
        global,
        token::args::{self, OutputFormat},
    },
    config::{self, network, token::UnresolvedToken, UnresolvedScAddress},
    output::Output,
};

#[derive(Debug, Parser, Clone)]
#[group(skip)]
pub struct Cmd {
    /// The token to re-administer: a contract id or alias, or a classic asset as
    /// `CODE:ISSUER`.
    #[arg(long = "id")]
    pub id: UnresolvedToken,

    /// The new administrator to hand control to. Accepts a `G…` account, a `C…`
    /// contract address, or an alias.
    #[arg(long = "new-admin")]
    pub new_admin: UnresolvedScAddress,

    /// Format of the output.
    #[arg(long, default_value = "text")]
    pub output: OutputFormat,

    #[command(flatten)]
    pub config: config::Args,
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

    #[error("muxed (M…) source accounts are not yet supported for `token set-admin`")]
    MuxedSourceNotSupported,

    #[error("muxed (M…) new-admin accounts are not yet supported for `token set-admin`")]
    MuxedNewAdminNotSupported,
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
            Error::MuxedSourceNotSupported | Error::MuxedNewAdminNotSupported => "unsupported",
        }
    }
}

/// The machine-readable receipt of a set-admin change.
#[derive(Debug, serde::Serialize)]
struct Receipt {
    /// Hex-encoded hash of the submitted transaction.
    tx_hash: Option<String>,
    /// The decoded contract return value (`null` for the SAC `set_admin`, which
    /// returns nothing).
    result: serde_json::Value,
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let output = Output::new(self.output.into(), global_args.quiet);
        // In JSON mode the underlying invoke pipeline's human-readable status
        // logging (which writes to stderr) would still fire; run it quietly so
        // machine consumers get clean output without needing `--quiet`.
        let quiet = global_args.quiet || output.is_json();
        let config = &self.config;
        let network = config.get_network()?;

        let token = self
            .id
            .resolve(&config.locator, &network.network_passphrase)?;

        // The `--source` account only authorizes the change; it is not itself a
        // `set_admin` argument. The invoke pipeline can't source a transaction from
        // a muxed account yet (see #2645), so reject one up front with a clear
        // message.
        let source_account = config.source_account()?;
        if matches!(source_account, crate::xdr::MuxedAccount::MuxedEd25519(_)) {
            return Err(Error::MuxedSourceNotSupported);
        }
        // `set_admin` is a SAC-admin function; warn (in human-readable mode) if
        // the target isn't actually a Stellar Asset Contract.
        if !output.is_json() {
            args::warn_if_not_sac(output.print(), "set_admin", &token.contract_id, &network).await;
        }
        // `--new-admin` may be an account (`G…`), a contract (`C…`), or an alias.
        // A muxed (`M…`) successor would be stranded — it can't sign as a source
        // (see #2645) and the SAC stores a plain `Address` — so reject one up
        // front rather than performing an irreversible transfer to an
        // unusable admin.
        if self
            .new_admin
            .is_muxed(&config.locator, &network.network_passphrase)
        {
            return Err(Error::MuxedNewAdminNotSupported);
        }
        // Resolve it to an `ScAddress` and hand the strkey to the `set_admin`
        // arg, which accepts any of these administrators.
        let new_admin = self
            .new_admin
            .clone()
            .resolve(&config.locator, &network.network_passphrase, None)?
            .to_string();

        // SAC `set_admin(new_admin)` — supply the value and let the contract's
        // parameter be matched by position. A set-admin always intends to
        // submit, so force `Send::Yes`: a token whose `set_admin` records no
        // writes/events/auth can't be classified read-only and silently exit 0
        // without ever changing the administrator.
        let receipt = args::invoke_by_position(
            config,
            quiet,
            global_args.no_cache,
            &token,
            "set_admin",
            vec![new_admin],
            invoke::Send::Yes,
        )
        .await
        .map_err(|e| args::not_deployed_error(&token, &e).map_or(Error::Invoke(e), Error::Args))?
        .into_result();

        // `set_admin` always writes, so the invocation is submitted rather than
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
