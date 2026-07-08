use std::ffi::OsString;

use clap::Parser;

use crate::{
    commands::{
        contract::invoke,
        global,
        token::args::{self, OutputFormat, TokenTarget},
    },
    config::{self, locator, network, sign_with, UnresolvedContract, UnresolvedMuxedAccount},
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

    /// Account to transfer the tokens to.
    #[arg(long)]
    pub to: UnresolvedMuxedAccount,

    /// Amount to transfer, in the token's smallest unit (stroops for a Stellar
    /// Asset Contract).
    #[arg(long)]
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
    Invoke(#[from] invoke::Error),
    #[error(transparent)]
    Serde(#[from] serde_json::Error),

    #[error(
        "the Stellar Asset Contract {0} is not deployed on this network.\n\
         Deploy it first with `stellar contract asset deploy --asset <ASSET>`, then retry."
    )]
    SacNotDeployed(String),

    #[error("contract {0} was not found on this network")]
    ContractNotFound(String),
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

        let contract_id = self
            .id
            .resolve_contract_id(&config.locator, &network.network_passphrase)?;

        // SEP-41 `transfer(from, to, amount)`: `from` is the source account
        // (which also signs and authorizes), `to` is the destination.
        let from = config.source_account()?.to_string();
        let to = self
            .to
            .resolve_muxed_account(&config.locator, None)
            .map_err(config::Error::from)?
            .to_string();
        let amount = self.amount.to_string();

        let slop: Vec<OsString> = [
            "transfer", "--from", &from, "--to", &to, "--amount", &amount,
        ]
        .into_iter()
        .map(OsString::from)
        .collect();

        let invoke_cmd = invoke::Cmd {
            contract_id: UnresolvedContract::Resolved(contract_id),
            slop,
            config: config.clone(),
            ..Default::default()
        };

        let receipt = invoke_cmd
            .execute_with_receipt(&config, quiet, global_args.no_cache)
            .await
            .map_err(|e| self.map_invoke_error(e, &contract_id))?
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

    /// Translate a raw invocation failure into a token-aware error where we can
    /// recognize it. A missing contract instance surfaces as a
    /// `Contract not found` RPC error while fetching the spec; for an asset
    /// target that means the SAC has not been deployed yet, so we point the user
    /// at `contract asset deploy`. Other failures pass through unchanged.
    fn map_invoke_error(
        &self,
        err: invoke::Error,
        contract_id: &stellar_strkey::Contract,
    ) -> Error {
        use crate::{get_spec, rpc};

        if let invoke::Error::GetSpecError(get_spec::Error::Rpc(rpc::Error::NotFound(kind, _))) =
            &err
        {
            if kind == "Contract" {
                return if matches!(self.id, TokenTarget::Asset(_)) {
                    Error::SacNotDeployed(format!("{contract_id}"))
                } else {
                    Error::ContractNotFound(format!("{contract_id}"))
                };
            }
        }

        Error::Invoke(err)
    }
}
