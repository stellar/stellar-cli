use crate::{snapshot, strval::StrValError};

use soroban_env_host::{
    xdr::{Error as XdrError, ScVal},
    HostError,
};
use soroban_spec::gen::{json, rust};

use hex::FromHexError;
use std::io;

#[derive(thiserror::Error, Debug)]
pub enum CmdError {
    #[error("parsing argument {arg}: {error}")]
    CannotParseArg { arg: String, error: StrValError },
    #[error("parsing key {key}: {error}")]
    CannotParseKey { key: String, error: StrValError },
    #[error("parsing XDR key {key}: {error}")]
    CannotParseXDRKey { key: String, error: XdrError },
    #[error("parsing XDR key {arg}: {error}")]
    CannotParseXDRArg { arg: String, error: XdrError },
    #[error("missing key argument")]
    MissingKey,
    #[error("cannot add contract to ledger entries: {0}")]
    CannotAddContractToLedgerEntries(XdrError),
    #[error(transparent)]
    // TODO: the Display impl of host errors is pretty user-unfriendly
    //       (it just calls Debug). I think we can do better than that
    Host(#[from] HostError),
    #[error("reading file {filepath}: {error}")]
    CannotReadLedgerFile {
        filepath: std::path::PathBuf,
        error: snapshot::Error,
    },
    #[error("reading file {filepath}: {error}")]
    CannotReadContractFile {
        filepath: std::path::PathBuf,
        error: io::Error,
    },
    #[error("cannot parse wasm file {file}: {error}")]
    CannotParseWasm {
        file: std::path::PathBuf,
        error: wasmparser::BinaryReaderError,
    },
    #[error("committing file {filepath}: {error}")]
    CannotCommitLedgerFile {
        filepath: std::path::PathBuf,
        error: snapshot::Error,
    },
    #[error("cannot parse contract ID {contract_id}: {error}")]
    CannotParseContractID {
        contract_id: String,
        error: FromHexError,
    },
    #[error("function {0} was not found in the contract")]
    FunctionNotFoundInContractSpec(String),
    #[error("unexpected number of arguments: {provided} (function {function} expects {expected} argument(s))")]
    UnexpectedArgumentCount {
        provided: usize,
        expected: usize,
        function: String,
    },
    #[error("function name {0} is too long")]
    FunctionNameTooLong(String),
    #[error("argument count ({current}) surpasses maximum allowed count ({maximum})")]
    MaxNumberOfArgumentsReached { current: usize, maximum: usize },
    #[error("cannot print result {result:?}: {error}")]
    CannotPrintResult { result: ScVal, error: StrValError },
    #[error("cannot print result {result:?}: {error}")]
    CannotPrintJSONResult {
        result: ScVal,
        error: serde_json::Error,
    },
    #[error("xdr processing error: {0}")]
    Xdr(#[from] XdrError),
    #[error("generate rust from file: {0}")]
    CannotGenerateRustFromFile(rust::GenerateFromFileError),
    #[error("format rust error: {0}")]
    CannotFormatRust(syn::Error),
    #[error("generate json from file: {0}")]
    CannotGenerateJSONFromFile(json::GenerateFromFileError),
}
