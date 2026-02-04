use std::convert::{Infallible, TryInto};
use std::ffi::OsString;
use std::num::ParseIntError;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::{fmt::Debug, fs, io};

use clap::{Parser, ValueEnum};
use soroban_rpc::{Client, SimulateHostFunctionResult, SimulateTransactionResponse};
use soroban_spec::read::FromWasmError;

use super::super::events;
use super::arg_parsing;
use crate::assembled::Assembled;
use crate::commands::tx::fetch;
use crate::log::extract_events;
use crate::print::Print;
use crate::utils::deprecate_message;
use crate::{
    assembled::simulate_and_assemble_transaction,
    commands::{
        contract::arg_parsing::{build_host_function_parameters, output_to_string},
        global,
        tx::fetch::fee,
        txn_result::{TxnEnvelopeResult, TxnResult},
    },
    config::{self, data, locator, network},
    get_spec::{self, get_remote_contract_spec},
    print, rpc,
    xdr::{
        self, AccountEntry, AccountEntryExt, AccountId, ContractEvent, ContractEventType,
        DiagnosticEvent, HostFunction, InvokeContractArgs, InvokeHostFunctionOp, Limits, Memo,
        MuxedAccount, Operation, OperationBody, Preconditions, PublicKey, ScSpecEntry,
        SequenceNumber, String32, StringM, Thresholds, Transaction, TransactionExt, Uint256, VecM,
        WriteXdr,
    },
    Pwd,
};
use soroban_spec_tools::contract;

#[derive(Parser, Debug, Default, Clone)]
#[allow(clippy::struct_excessive_bools)]
#[group(skip)]
pub struct Cmd {
    /// Contract ID to invoke
    #[arg(long = "id", env = "STELLAR_CONTRACT_ID")]
    pub contract_id: config::UnresolvedContract,

    // For testing only
    #[arg(skip)]
    pub wasm: Option<std::path::PathBuf>,

    /// ⚠️ Deprecated, use `--send=no`. View the result simulating and do not sign and submit transaction.
    #[arg(long, env = "STELLAR_INVOKE_VIEW")]
    pub is_view: bool,

    /// Function name as subcommand, then arguments for that function as `--arg-name value`
    #[arg(last = true, id = "CONTRACT_FN_AND_ARGS")]
    pub slop: Vec<OsString>,

    #[command(flatten)]
    pub config: config::Args,

    #[command(flatten)]
    pub resources: crate::resources::Args,

    /// Whether or not to send a transaction
    #[arg(long, value_enum, default_value_t, env = "STELLAR_SEND")]
    pub send: Send,

    /// Build the transaction and only write the base64 xdr to stdout
    #[arg(long)]
    pub build_only: bool,
}

impl FromStr for Cmd {
    type Err = clap::error::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use clap::{CommandFactory, FromArgMatches};
        Self::from_arg_matches_mut(&mut Self::command().get_matches_from(s.split_whitespace()))
    }
}

impl Pwd for Cmd {
    fn set_pwd(&mut self, pwd: &Path) {
        self.config.set_pwd(pwd);
    }
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("cannot add contract to ledger entries: {0}")]
    CannotAddContractToLedgerEntries(xdr::Error),

    #[error("reading file {0:?}: {1}")]
    CannotReadContractFile(PathBuf, io::Error),

    #[error("committing file {filepath}: {error}")]
    CannotCommitEventsFile {
        filepath: std::path::PathBuf,
        error: events::Error,
    },

    #[error("parsing contract spec: {0}")]
    CannotParseContractSpec(FromWasmError),

    #[error(transparent)]
    Xdr(#[from] xdr::Error),

    #[error("error parsing int: {0}")]
    ParseIntError(#[from] ParseIntError),

    #[error(transparent)]
    Rpc(#[from] rpc::Error),

    #[error("missing operation result")]
    MissingOperationResult,

    #[error("error loading signing key: {0}")]
    SignatureError(#[from] ed25519_dalek::SignatureError),

    #[error(transparent)]
    Config(#[from] config::Error),

    #[error("unexpected ({length}) simulate transaction result length")]
    UnexpectedSimulateTransactionResultSize { length: usize },

    #[error(transparent)]
    Clap(#[from] clap::Error),

    #[error(transparent)]
    Locator(#[from] locator::Error),

    #[error("{message}")]
    ContractInvoke {
        /// Full error message with the resolved error name inserted after the
        /// contract error code.
        message: String,
        /// The resolved error name and doc string (e.g. `"ErrorName: description"`).
        detail: String,
    },

    #[error(transparent)]
    StrKey(#[from] stellar_strkey::DecodeError),

    #[error(transparent)]
    ContractSpec(#[from] contract::Error),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Data(#[from] data::Error),

    #[error(transparent)]
    Network(#[from] network::Error),

    #[error(transparent)]
    GetSpecError(#[from] get_spec::Error),

    #[error(transparent)]
    ArgParsing(#[from] arg_parsing::Error),

    #[error(transparent)]
    Fee(#[from] fee::Error),

    #[error(transparent)]
    Fetch(#[from] fetch::Error),
}

impl From<Infallible> for Error {
    fn from(_: Infallible) -> Self {
        unreachable!()
    }
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let print = Print::new(global_args.quiet);
        let res = self.invoke(global_args).await?.to_envelope();

        if self.is_view {
            deprecate_message(print, "--is-view", "Use `--send=no` instead.");
        }

        match res {
            TxnEnvelopeResult::TxnEnvelope(tx) => println!("{}", tx.to_xdr_base64(Limits::none())?),
            TxnEnvelopeResult::Res(output) => {
                println!("{output}");
            }
        }
        Ok(())
    }

    pub async fn invoke(&self, global_args: &global::Args) -> Result<TxnResult<String>, Error> {
        self.execute(&self.config, global_args.quiet, global_args.no_cache)
            .await
    }

    pub fn read_wasm(&self) -> Result<Option<Vec<u8>>, Error> {
        Ok(if let Some(wasm) = self.wasm.as_ref() {
            Some(fs::read(wasm).map_err(|e| Error::CannotReadContractFile(wasm.clone(), e))?)
        } else {
            None
        })
    }

    pub fn spec_entries(&self) -> Result<Option<Vec<ScSpecEntry>>, Error> {
        self.read_wasm()?
            .map(|wasm| {
                soroban_spec::read::from_wasm(&wasm).map_err(Error::CannotParseContractSpec)
            })
            .transpose()
    }

    fn should_send_tx(&self, sim_res: &SimulateTransactionResponse) -> Result<ShouldSend, Error> {
        Ok(match self.send {
            Send::Default => {
                if self.is_view {
                    ShouldSend::No
                } else if has_write(sim_res)? || has_published_event(sim_res)? || has_auth(sim_res)?
                {
                    ShouldSend::Yes
                } else {
                    ShouldSend::DefaultNo
                }
            }
            Send::No => ShouldSend::No,
            Send::Yes => ShouldSend::Yes,
        })
    }

    /// Uses a default account to check if the tx should be sent after the simulation. The transaction
    /// should be recreated with the real source account later.
    async fn simulate(
        &self,
        host_function_params: &InvokeContractArgs,
        account_details: &AccountEntry,
        rpc_client: &Client,
    ) -> Result<Assembled, Error> {
        let sequence: i64 = account_details.seq_num.0;
        let AccountId(PublicKey::PublicKeyTypeEd25519(account_id)) =
            account_details.account_id.clone();

        let tx =
            build_invoke_contract_tx(host_function_params.clone(), sequence + 1, 100, account_id)?;
        Ok(simulate_and_assemble_transaction(
            rpc_client,
            &tx,
            self.resources.resource_config(),
            self.resources.resource_fee,
        )
        .await?)
    }

    #[allow(clippy::too_many_lines)]
    pub async fn execute(
        &self,
        config: &config::Args,
        quiet: bool,
        no_cache: bool,
    ) -> Result<TxnResult<String>, Error> {
        let print = print::Print::new(quiet);
        let network = config.get_network()?;

        tracing::trace!(?network);

        let contract_id = self
            .contract_id
            .resolve_contract_id(&config.locator, &network.network_passphrase)?;

        let spec_entries = self.spec_entries()?;

        if let Some(spec_entries) = &spec_entries {
            // For testing wasm arg parsing
            build_host_function_parameters(&contract_id, &self.slop, spec_entries, config).await?;
        }

        let client = network.rpc_client()?;

        let global_args = global::Args {
            locator: config.locator.clone(),
            filter_logs: Vec::default(),
            quiet,
            verbose: false,
            very_verbose: false,
            list: false,
            no_cache,
        };

        let spec_entries = get_remote_contract_spec(
            &contract_id.0,
            &config.locator,
            &config.network,
            Some(&global_args),
            Some(config),
        )
        .await
        .map_err(Error::from)?;

        let params =
            build_host_function_parameters(&contract_id, &self.slop, &spec_entries, config).await?;

        let (function, spec, host_function_params, signers) = params;

        // `self.build_only` will be checked again below and the fn will return a TxnResult::Txn
        // if the user passed the --build-only flag
        let (should_send, cached_simulation) = if self.build_only {
            (ShouldSend::Yes, None)
        } else {
            let assembled = self
                .simulate(&host_function_params, &default_account_entry(), &client)
                .await
                .map_err(|e| enhance_error(e, &spec))?;
            let should_send = self.should_send_tx(&assembled.sim_res)?;
            (should_send, Some(assembled))
        };

        let account_details = if should_send == ShouldSend::Yes {
            client
                .verify_network_passphrase(Some(&network.network_passphrase))
                .await?;

            client
                .get_account(&config.source_account().await?.to_string())
                .await?
        } else {
            if should_send == ShouldSend::DefaultNo {
                print.infoln(
                    "Simulation identified as read-only. Send by rerunning with `--send=yes`.",
                );
            }

            let assembled = cached_simulation.expect(
                "cached_simulation should be available when should_send != Yes and not build_only",
            );
            let sim_res = assembled.sim_response();
            let return_value = sim_res.results()?;
            let events = sim_res.events()?;

            crate::log::event::all(&events);
            crate::log::event::contract(&events, &print);

            return Ok(output_to_string(&spec, &return_value[0].xdr, &function)?);
        };

        let sequence: i64 = account_details.seq_num.into();
        let AccountId(PublicKey::PublicKeyTypeEd25519(account_id)) = account_details.account_id;

        let tx = Box::new(build_invoke_contract_tx(
            host_function_params.clone(),
            sequence + 1,
            config.get_inclusion_fee()?,
            account_id,
        )?);

        if self.build_only {
            return Ok(TxnResult::Txn(tx));
        }

        let txn = simulate_and_assemble_transaction(
            &client,
            &tx,
            self.resources.resource_config(),
            self.resources.resource_fee,
        )
        .await
        .map_err(|e| enhance_error(Error::Rpc(e), &spec))?;
        let assembled = self.resources.apply_to_assembled_txn(txn);
        let mut txn = Box::new(assembled.transaction().clone());
        let sim_res = assembled.sim_response();

        if !no_cache {
            data::write(sim_res.clone().into(), &network.rpc_uri()?)?;
        }

        // Need to sign all auth entries
        if let Some(tx) = config.sign_soroban_authorizations(&txn, &signers).await? {
            *txn = tx;
        }

        let res = client
            .send_transaction_polling(&config.sign(*txn, quiet).await?)
            .await
            .map_err(|e| enhance_error(Error::Rpc(e), &spec))?;

        self.resources.print_cost_info(&res)?;

        if !no_cache {
            data::write(res.clone().try_into()?, &network.rpc_uri()?)?;
        }

        let return_value = res.return_value()?;
        let events = extract_events(&res.result_meta.unwrap_or_default());

        crate::log::event::all(&events);
        crate::log::event::contract(&events, &print);

        Ok(output_to_string(&spec, &return_value, &function)?)
    }
}

const DEFAULT_ACCOUNT_ID: AccountId = AccountId(PublicKey::PublicKeyTypeEd25519(Uint256([0; 32])));

fn default_account_entry() -> AccountEntry {
    AccountEntry {
        account_id: DEFAULT_ACCOUNT_ID,
        balance: 0,
        seq_num: SequenceNumber(0),
        num_sub_entries: 0,
        inflation_dest: None,
        flags: 0,
        home_domain: String32::from(unsafe { StringM::<32>::from_str("TEST").unwrap_unchecked() }),
        thresholds: Thresholds([0; 4]),
        signers: unsafe { [].try_into().unwrap_unchecked() },
        ext: AccountEntryExt::V0,
    }
}

fn build_invoke_contract_tx(
    parameters: InvokeContractArgs,
    sequence: i64,
    fee: u32,
    source_account_id: Uint256,
) -> Result<Transaction, Error> {
    let op = Operation {
        source_account: None,
        body: OperationBody::InvokeHostFunction(InvokeHostFunctionOp {
            host_function: HostFunction::InvokeContract(parameters),
            auth: VecM::default(),
        }),
    };
    Ok(Transaction {
        source_account: MuxedAccount::Ed25519(source_account_id),
        fee,
        seq_num: SequenceNumber(sequence),
        cond: Preconditions::None,
        memo: Memo::None,
        operations: vec![op].try_into()?,
        ext: TransactionExt::V0,
    })
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, ValueEnum, Default)]
pub enum Send {
    /// Send transaction if simulation indicates there are ledger writes,
    /// published events, or auth required, otherwise return simulation result
    #[default]
    Default,
    /// Do not send transaction, return simulation result
    No,
    /// Always send transaction
    Yes,
}

#[derive(Debug, PartialEq)]
enum ShouldSend {
    DefaultNo,
    No,
    Yes,
}

/// Extract a contract error code (u32) from an error string.
///
/// Supports two formats:
/// - `Error(Contract, #N)` from the Soroban host display format (simulation errors)
/// - `Contract(N)` from Rust Debug format of `ScError::Contract(u32)` (submission errors)
///
/// The Display format uses the prefix `Contract, #` to distinguish contract errors
/// from other Soroban error types (Budget, Auth, etc.) which also use `#N`.
///
/// The Debug format is used by `TransactionSubmissionFailed` errors which
/// pretty-print (`{:#?}`) the `TransactionResult`, where the number may
/// appear on a separate line with surrounding whitespace.
fn extract_contract_error_code(msg: &str) -> Option<u32> {
    // Try `Contract, #N` format (simulation errors).
    // Must match the full prefix to avoid false positives on non-contract
    // error types like `Error(Budget, #3)`.
    if let Some(idx) = msg.find("Contract, #") {
        let after = &msg[idx + "Contract, #".len()..];
        let end = after
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(after.len());
        if end > 0 {
            if let Ok(code) = after[..end].parse() {
                return Some(code);
            }
        }
    }

    // Try `Contract(N)` format (transaction submission errors via Debug).
    // In the Debug-printed XDR, `ScError::Contract(u32)` is the only variant
    // that uses `Contract(` followed by a number.
    if let Some(idx) = msg.find("Contract(") {
        let after = &msg[idx + "Contract(".len()..];
        let trimmed = after.trim_start();
        let end = trimmed
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(trimmed.len());
        if end > 0 {
            if let Ok(code) = trimmed[..end].parse() {
                return Some(code);
            }
        }
    }

    None
}

/// Try to enhance an error with human-readable contract error information from
/// the contract spec. If the error contains a contract error code — either
/// `#N` from simulation errors or `Contract(N)` from transaction submission
/// errors — looks it up across all error enums in the spec and returns a
/// `ContractInvoke` error with the resolved name and documentation.
///
/// The resolved error name is inserted into the error message right after the
/// error code, so it appears next to `Error(Contract, #N)` rather than being
/// separated from it by the event log.
///
/// Returns the original error unchanged if enhancement is not possible.
fn enhance_error(err: Error, spec: &soroban_spec_tools::Spec) -> Error {
    let error_msg = match &err {
        Error::Rpc(rpc_err) => rpc_err.to_string(),
        _ => return err,
    };

    let Some(code) = extract_contract_error_code(&error_msg) else {
        return err;
    };

    let Some((_enum_info, case)) = spec.find_error_type_any(code) else {
        return err;
    };

    let name = case.name.to_utf8_string_lossy();
    let doc = case.doc.to_utf8_string_lossy();
    let detail = format!(
        "{name}{}",
        if doc.is_empty() {
            String::new()
        } else {
            format!(": {doc}")
        }
    );

    let enhanced_msg = insert_detail_after_error_code(&error_msg, &detail);
    Error::ContractInvoke {
        message: enhanced_msg,
        detail,
    }
}

/// Insert a detail string into an error message right after the contract error
/// code line, before the event log section.
///
/// The RPC simulation error typically has the error on the first line, followed
/// by a blank line (`\n\n`) and then the "Event log (newest first):" section.
/// This function inserts the detail between the error line and the event log so
/// the resolved error name appears next to the error code.
///
/// If no blank line separator is found, the detail is appended at the end.
fn insert_detail_after_error_code(msg: &str, detail: &str) -> String {
    if let Some(pos) = msg.find("\n\n") {
        format!("{}\n{}{}", &msg[..pos], detail, &msg[pos..])
    } else {
        format!("{msg}\n{detail}")
    }
}

fn has_write(sim_res: &SimulateTransactionResponse) -> Result<bool, Error> {
    Ok(!sim_res
        .transaction_data()?
        .resources
        .footprint
        .read_write
        .is_empty())
}

fn has_published_event(sim_res: &SimulateTransactionResponse) -> Result<bool, Error> {
    Ok(sim_res.events()?.iter().any(
        |DiagnosticEvent {
             event: ContractEvent { type_, .. },
             ..
         }| matches!(type_, ContractEventType::Contract),
    ))
}

fn has_auth(sim_res: &SimulateTransactionResponse) -> Result<bool, Error> {
    Ok(sim_res
        .results()?
        .iter()
        .any(|SimulateHostFunctionResult { auth, .. }| !auth.is_empty()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_code_from_simulation_format() {
        let msg = "transaction simulation failed: HostError: Error(Contract, #1)";
        assert_eq!(extract_contract_error_code(msg), Some(1));
    }

    #[test]
    fn extract_code_from_debug_format() {
        // Debug format from TransactionSubmissionFailed errors, which pretty-print
        // the TransactionResult XDR containing ScError::Contract(u32).
        assert_eq!(
            extract_contract_error_code("transaction submission failed: Contract(1)"),
            Some(1),
        );
        // Pretty-printed variant with whitespace around the number.
        assert_eq!(
            extract_contract_error_code("Err(\n    Contract(\n        1,\n    ),\n)"),
            Some(1),
        );
    }

    #[test]
    fn extract_code_ignores_non_contract_errors() {
        // Budget errors also use `#N` but should not match.
        assert_eq!(
            extract_contract_error_code(
                "transaction simulation failed: HostError: Error(Budget, #3)"
            ),
            None,
        );
        // Bare `#N` without the `Contract, ` prefix should not match.
        assert_eq!(extract_contract_error_code("something #123 happened"), None);
    }
}
