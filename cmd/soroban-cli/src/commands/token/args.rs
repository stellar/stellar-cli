use crate::{
    commands::contract::invoke, config::token::ResolvedToken, get_spec, output::Format, rpc,
};

/// Output format shared by the `stellar token` subcommands.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, clap::ValueEnum, Default)]
pub enum OutputFormat {
    /// Human-readable text.
    #[default]
    Text,
    /// Compact, single-line JSON receipt.
    Json,
    /// Formatted (multiline) JSON receipt.
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
}

impl Error {
    /// Machine-readable discriminator for the JSON error envelope's `type` field.
    #[must_use]
    pub fn error_type(&self) -> &'static str {
        match self {
            Error::SacNotDeployed(_) => "sac_not_deployed",
            Error::ContractNotFound(_) => "contract_not_found",
        }
    }
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
