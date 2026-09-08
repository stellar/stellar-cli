use crate::{
    commands::{contract::invoke, txn_result::TxnResult},
    config::{self, token::ResolvedToken, UnresolvedContract},
    get_spec,
    output::Format,
    rpc,
};

/// Output format shared by the `stellar token` subcommands.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, clap::ValueEnum, Default)]
pub enum OutputFormat {
    /// Human-readable text.
    #[default]
    Text,
    /// Compact, single-line JSON output.
    Json,
    /// Formatted (multiline) JSON output.
    JsonFormatted,
}

impl From<OutputFormat> for Format {
    fn from(value: OutputFormat) -> Self {
        match value {
            OutputFormat::Text => Format::Readable,
            OutputFormat::Json => Format::Json,
            OutputFormat::JsonFormatted => Format::JsonFormatted,
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(
        "the Stellar Asset Contract {0} is not deployed on this network.\n\
         Deploy it first with `stellar contract asset deploy --asset <ASSET>`, then retry."
    )]
    SacNotDeployed(String),

    #[error("contract {0} was not found on this network")]
    ContractNotFound(String),

    #[error(
        "the token reports {decimals} decimals, which exceeds the maximum {max} \
         this command can format.\n\
         Query the raw smallest-unit value without `--decimal`."
    )]
    DecimalsTooLarge { decimals: u32, max: u32 },
}

impl Error {
    /// Machine-readable discriminator for the JSON error envelope's `type` field.
    #[must_use]
    pub fn error_type(&self) -> &'static str {
        match self {
            Error::SacNotDeployed(_) => "sac_not_deployed",
            Error::ContractNotFound(_) => "contract_not_found",
            Error::DecimalsTooLarge { .. } => "decimals_too_large",
        }
    }
}

/// The largest `decimals` the token commands will render in `--decimal` form. A
/// token amount is an `i128` (at most 39 significant digits), so a larger scale
/// can't represent a meaningful integer part. The cap also matters for safety:
/// `decimals` is contract-controlled, and decimal formatting pads the output to
/// that many fractional characters, so an unbounded value from a hostile token
/// could otherwise force a multi-gigabyte allocation.
pub const MAX_DECIMALS: u32 = 38;

/// Render `value` scaled by `decimals` as a decimal string, refusing a scale
/// beyond [`MAX_DECIMALS`] so a contract-supplied `decimals` can't trigger a
/// pathological allocation. The raw value stays available without `--decimal`.
pub fn format_decimal(value: i128, decimals: u32) -> Result<String, Error> {
    if decimals > MAX_DECIMALS {
        return Err(Error::DecimalsTooLarge {
            decimals,
            max: MAX_DECIMALS,
        });
    }
    Ok(crate::fixed_point::FixedPoint::new(value, decimals).to_string())
}

/// Invoke a token `function` by SEP-41 canonical position: `args` are supplied
/// in the function's parameter order and mapped onto the contract's parameters
/// by index, so the call works regardless of what the contract names them.
///
/// Returns the raw `invoke::Error` so callers keep their own
/// `not_deployed_error` translation; a `None` result means `--build-only`, which
/// the token commands never set.
pub async fn invoke_by_position(
    config: &config::Args,
    quiet: bool,
    no_cache: bool,
    token: &ResolvedToken,
    function: &str,
    args: Vec<String>,
    send: invoke::Send,
) -> Result<TxnResult<invoke::InvokeReceipt>, invoke::Error> {
    let cmd = invoke::Cmd {
        contract_id: UnresolvedContract::Resolved(token.contract_id),
        invocation: Some(invoke::PositionalInvocation {
            function: function.to_string(),
            args,
        }),
        config: config.clone(),
        send,
        ..Default::default()
    };

    cmd.execute_with_receipt(config, quiet, no_cache).await
}

/// If `err` is a "contract not found" failure raised while fetching the contract
/// spec, translate it into a token-aware error keyed off what `token` resolved
/// to: a missing SAC (pointing at `contract asset deploy`), or a missing
/// contract for a direct id/alias. Returns `None` for any other failure so the
/// caller can surface the underlying invoke error unchanged.
#[must_use]
pub fn not_deployed_error(token: &ResolvedToken, err: &invoke::Error) -> Option<Error> {
    let invoke::Error::GetSpecError(get_spec::Error::Rpc(rpc::Error::NotFound(kind, _))) = err
    else {
        return None;
    };
    if kind != "Contract" {
        return None;
    }
    let contract_id = token.contract_id;
    Some(if token.is_sac() {
        Error::SacNotDeployed(format!("{contract_id}"))
    } else {
        Error::ContractNotFound(format!("{contract_id}"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_decimal_scales_within_bound() {
        assert_eq!(format_decimal(12_300_000, 7).unwrap(), "1.23");
        assert_eq!(format_decimal(1, 0).unwrap(), "1");
        // The boundary itself is allowed.
        assert!(format_decimal(1, MAX_DECIMALS).is_ok());
    }

    #[test]
    fn format_decimal_rejects_scale_beyond_bound() {
        let err = format_decimal(1, MAX_DECIMALS + 1).unwrap_err();
        assert_eq!(err.error_type(), "decimals_too_large");
    }
}
