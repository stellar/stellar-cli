use std::convert::{Infallible, TryInto};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::{fmt::Debug, fs, io};

use clap::{Parser, ValueEnum};
use soroban_rpc::{
    Client, GetTransactionResponse, SimulateHostFunctionResult, SimulateTransactionResponse,
};
use soroban_spec::read::FromWasmError;

use super::super::events;
use super::arg_parsing;
use crate::commands::tx::fetch;
use crate::log::extract_events;
use crate::print::Print;
use crate::utils::deprecate_message;
use crate::{
    assembled::{simulate_transaction, Assembled},
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
        self, AccountEntry, AccountEntryExt, AccountId, ContractEvent, ContractEventBody,
        ContractEventType, ContractEventV0, DiagnosticEvent, HostFunction, InvokeContractArgs,
        InvokeHostFunctionOp, Limits, Memo, MuxedAccount, Operation, OperationBody, Preconditions,
        PublicKey, ScError, ScSpecEntry, ScSpecTypeDef, ScSpecTypeUdt, ScVal, SequenceNumber,
        String32, StringM, Thresholds, Transaction, TransactionExt, Uint256, VecM, WriteXdr,
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
        spec: &soroban_spec_tools::Spec,
        function: &str,
    ) -> Result<Assembled, Error> {
        let sequence: i64 = account_details.seq_num.0;
        let AccountId(PublicKey::PublicKeyTypeEd25519(account_id)) =
            account_details.account_id.clone();

        let tx =
            build_invoke_contract_tx(host_function_params.clone(), sequence + 1, 100, account_id)?;
        simulate_and_enhance(
            rpc_client,
            &tx,
            self.resources.resource_config(),
            self.resources.resource_fee,
            spec,
            function,
        )
        .await
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
                .simulate(
                    &host_function_params,
                    &default_account_entry(),
                    &client,
                    &spec,
                    &function,
                )
                .await?;
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

        let txn = simulate_and_enhance(
            &client,
            &tx,
            self.resources.resource_config(),
            self.resources.resource_fee,
            &spec,
            &function,
        )
        .await?;
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

        let signed_tx = config.sign(*txn, quiet).await?;
        let hash = client.send_transaction(&signed_tx).await?;

        let res = match client.get_transaction_polling(&hash, None).await {
            Ok(res) => res,
            Err(e) => {
                // For submission failures, extract the contract error code
                if matches!(&e, rpc::Error::TransactionSubmissionFailed(_)) {
                    if let Ok(response) = client.get_transaction(&hash).await {
                        if let Some(err) =
                            enhance_error_from_meta(&response, &e.to_string(), &spec, &function)
                        {
                            return Err(err);
                        }
                    }
                }
                return Err(Error::Rpc(e));
            }
        };

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

async fn simulate_and_enhance(
    client: &Client,
    tx: &Transaction,
    resource_config: Option<soroban_rpc::ResourceConfig>,
    resource_fee: Option<i64>,
    spec: &soroban_spec_tools::Spec,
    function: &str,
) -> Result<Assembled, Error> {
    let sim_res = simulate_transaction(client, tx, resource_config).await?;

    if let Some(e) = &sim_res.error {
        if let Ok(events) = sim_res.events() {
            crate::log::event::all(&events);
        }
        return Err(enhance_simulation_error(e, &sim_res, spec, function));
    }

    Ok(Assembled::new(tx, sim_res, resource_fee)?)
}

/// Attempt to resolve a contract error code from the simulation response and
/// enhance the error message with the error name and documentation from the spec.
fn enhance_simulation_error(
    error_msg: &str,
    sim_res: &SimulateTransactionResponse,
    spec: &soroban_spec_tools::Spec,
    function: &str,
) -> Error {
    let events = sim_res.events().ok();

    // Non-try cross-contract calls that trap should not have their inner error
    // codes resolved against the outer function's error enum.
    if events.as_ref().is_some_and(|e| has_cross_contract_trap(e)) {
        return Error::Rpc(rpc::Error::TransactionSimulationFailed(
            error_msg.to_string(),
        ));
    }

    // Extract and resolve the contract error code from diagnostic events.
    if let Some(code) = events
        .as_ref()
        .and_then(|e| extract_contract_error_from_events(e))
    {
        if let Some(err) = build_enhanced_error(code, error_msg, spec, function) {
            return err;
        }
    }
    if let Some(code) = extract_contract_error_from_error_msg(error_msg) {
        if let Some(err) = build_enhanced_error(code, error_msg, spec, function) {
            return err;
        }
    }

    Error::Rpc(rpc::Error::TransactionSimulationFailed(
        error_msg.to_string(),
    ))
}

/// Detect non-try cross-contract call traps.
///
/// Both `panic_with_error!` and non-try cross-contract calls produce VM traps,
/// but with different host function names in the diagnostic message:
///   - `panic_with_error!`: "...host function call: fail_with_error"
///   - cross-contract trap: "...host function call: call"
///
/// We only want to suppress resolution for cross-contract traps (where the
/// error code belongs to an inner contract, not the invoked function's spec).
fn has_cross_contract_trap(events: &[DiagnosticEvent]) -> bool {
    const CROSS_CONTRACT_TRAP_MSG: &str =
        "escalating error to VM trap from failed host function call: call";

    events.iter().rev().any(|event| {
        let ContractEventBody::V0(ContractEventV0 { data, .. }) = &event.event.body;
        matches!(data, ScVal::String(s) if s.to_utf8_string_lossy().contains(CROSS_CONTRACT_TRAP_MSG))
    })
}

/// Extract the contract error code from diagnostic events.
///
/// Scans events (newest first) for the outermost contract error code
fn extract_contract_error_from_events(events: &[DiagnosticEvent]) -> Option<u32> {
    for event in events.iter().rev() {
        let ContractEventBody::V0(ContractEventV0 { topics, data, .. }) = &event.event.body;

        if let ScVal::Error(ScError::Contract(code)) = data {
            return Some(*code);
        }
        for topic in topics.iter() {
            if let ScVal::Error(ScError::Contract(code)) = topic {
                return Some(*code);
            }
        }
    }
    None
}

fn enhance_error_from_meta(
    response: &GetTransactionResponse,
    rpc_error_msg: &str,
    spec: &soroban_spec_tools::Spec,
    function: &str,
) -> Option<Error> {
    let Ok(ScVal::Error(ScError::Contract(code))) = response.return_value() else {
        return None;
    };
    build_enhanced_error(code, rpc_error_msg, spec, function)
}

fn build_enhanced_error(
    code: u32,
    error_msg: &str,
    spec: &soroban_spec_tools::Spec,
    function: &str,
) -> Option<Error> {
    let case = find_error_for_function(spec, function, code)?;
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

    let enhanced_msg = insert_detail_after_error_code(error_msg, &detail);
    Some(Error::ContractInvoke {
        message: enhanced_msg,
        detail,
    })
}

fn find_error_for_function<'a>(
    spec: &'a soroban_spec_tools::Spec,
    function: &str,
    code: u32,
) -> Option<&'a xdr::ScSpecUdtErrorEnumCaseV0> {
    let func = spec.find_function(function).ok()?;
    let output = func.outputs.first()?;
    let ScSpecTypeDef::Result(result_type) = output else {
        return None;
    };
    let error_type = result_type.error_type.as_ref();
    let name = match error_type {
        ScSpecTypeDef::Udt(ScSpecTypeUdt { name }) => name,
        ScSpecTypeDef::Error => {
            return spec.find_error_type(code).ok();
        }
        _ => {
            return None;
        }
    };
    let error_enum_name = name.to_utf8_string_lossy();
    let error_entry = spec.find(&error_enum_name).ok()?;
    let ScSpecEntry::UdtErrorEnumV0(error_enum) = error_entry else {
        return None;
    };
    error_enum.cases.iter().find(|c| c.value == code)
}

fn insert_detail_after_error_code(msg: &str, detail: &str) -> String {
    if let Some(pos) = msg.find("\n\n") {
        format!("{}\n{}{}", &msg[..pos], detail, &msg[pos..])
    } else {
        format!("{msg}\n{detail}")
    }
}

fn extract_contract_error_from_error_msg(msg: &str) -> Option<u32> {
    let first_line = msg.lines().next().unwrap_or(msg);
    let marker = "Error(Contract, #";
    let start = first_line.find(marker)? + marker.len();
    let end = first_line[start..].find(')')?;
    first_line[start..start + end].parse::<u32>().ok()
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
    use super::extract_contract_error_from_error_msg;

    #[test]
    fn extracts_contract_error_code_from_first_line() {
        let msg = "HostError: Error(Contract, #5)\nEvent log (newest first):\n  0: ...";
        assert_eq!(extract_contract_error_from_error_msg(msg), Some(5));
    }

    #[test]
    fn ignores_contract_error_code_if_not_on_first_line() {
        let msg = "HostError: Error(WasmVm, InvalidAction)\nEvent log: Error(Contract, #5)";
        assert_eq!(extract_contract_error_from_error_msg(msg), None);
    }
}
