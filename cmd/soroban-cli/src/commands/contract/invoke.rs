use std::collections::HashMap;
use std::convert::{Infallible, TryInto};
use std::ffi::OsString;
use std::num::ParseIntError;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::{fmt::Debug, fs, io, rc::Rc};

use clap::{arg, command, Parser};
use heck::ToKebabCase;
use soroban_env_host::{
    budget::Budget,
    events::HostEvent,
    storage::Storage,
    xdr::{
        self, AccountId, Error as XdrError, Hash, HostFunction, InvokeHostFunctionOp,
        LedgerEntryData, LedgerFootprint, LedgerKey, LedgerKeyAccount, Memo, MuxedAccount,
        Operation, OperationBody, Preconditions, PublicKey, ScAddress, ScSpecEntry,
        ScSpecFunctionV0, ScSpecTypeDef, ScVal, ScVec, SequenceNumber, SorobanAddressCredentials,
        SorobanAuthorizationEntry, SorobanCredentials, Transaction, TransactionExt, Uint256, VecM,
    },
    DiagnosticLevel, Host, HostError,
};

use soroban_spec::read::FromWasmError;
use stellar_strkey::DecodeError;

use super::super::{
    config::{self, events_file, locator},
    events,
};
use crate::{
    commands::HEADING_SANDBOX,
    rpc::{self, Client},
    utils::{self, contract_spec, create_ledger_footprint, default_account_ledger_entry},
    Pwd,
};
use soroban_spec_tools::Spec;

#[derive(Parser, Debug, Default, Clone)]
#[allow(clippy::struct_excessive_bools)]
#[group(skip)]
pub struct Cmd {
    /// Contract ID to invoke
    #[arg(long = "id", env = "SOROBAN_CONTRACT_ID")]
    pub contract_id: String,
    /// WASM file of the contract to invoke (if using sandbox will deploy this file)
    #[arg(long)]
    pub wasm: Option<std::path::PathBuf>,

    /// Output the cost execution to stderr
    #[arg(long = "cost", conflicts_with = "rpc_url", conflicts_with="network", help_heading = HEADING_SANDBOX)]
    pub cost: bool,
    /// Run with an unlimited budget
    #[arg(long = "unlimited-budget",
          conflicts_with = "rpc_url",
          conflicts_with = "network",
          help_heading = HEADING_SANDBOX)]
    pub unlimited_budget: bool,

    // Function name as subcommand, then arguments for that function as `--arg-name value`
    #[arg(last = true, id = "CONTRACT_FN_AND_ARGS")]
    pub slop: Vec<OsString>,

    #[command(flatten)]
    pub config: config::Args,
    #[command(flatten)]
    pub events_file: events_file::Args,
    #[command(flatten)]
    pub fee: crate::fee::Args,
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
    #[error("parsing argument {arg}: {error}")]
    CannotParseArg {
        arg: String,
        error: soroban_spec_tools::Error,
    },
    #[error("cannot add contract to ledger entries: {0}")]
    CannotAddContractToLedgerEntries(XdrError),
    #[error(transparent)]
    // TODO: the Display impl of host errors is pretty user-unfriendly
    //       (it just calls Debug). I think we can do better than that
    Host(#[from] HostError),
    #[error("reading file {0:?}: {1}")]
    CannotReadContractFile(PathBuf, io::Error),
    #[error("committing file {filepath}: {error}")]
    CannotCommitEventsFile {
        filepath: std::path::PathBuf,
        error: events::Error,
    },
    #[error("cannot parse contract ID {0}: {1}")]
    CannotParseContractId(String, DecodeError),
    #[error("function {0} was not found in the contract")]
    FunctionNotFoundInContractSpec(String),
    #[error("parsing contract spec: {0}")]
    CannotParseContractSpec(FromWasmError),
    // },
    #[error("function name {0} is too long")]
    FunctionNameTooLong(String),
    #[error("argument count ({current}) surpasses maximum allowed count ({maximum})")]
    MaxNumberOfArgumentsReached { current: usize, maximum: usize },
    #[error("cannot print result {result:?}: {error}")]
    CannotPrintResult {
        result: ScVal,
        error: soroban_spec_tools::Error,
    },
    #[error(transparent)]
    Xdr(#[from] XdrError),
    #[error("error parsing int: {0}")]
    ParseIntError(#[from] ParseIntError),
    #[error(transparent)]
    Rpc(#[from] rpc::Error),
    #[error("unexpected contract code data type: {0:?}")]
    UnexpectedContractCodeDataType(LedgerEntryData),
    #[error("missing operation result")]
    MissingOperationResult,
    #[error("missing result")]
    MissingResult,
    #[error(transparent)]
    StrVal(#[from] soroban_spec_tools::Error),
    #[error(transparent)]
    Config(#[from] config::Error),
    #[error("unexpected ({length}) simulate transaction result length")]
    UnexpectedSimulateTransactionResultSize { length: usize },
    #[error("Missing argument {0}")]
    MissingArgument(String),
    #[error(transparent)]
    Clap(#[from] clap::Error),
    #[error(transparent)]
    Events(#[from] events_file::Error),
    #[error(transparent)]
    Locator(#[from] locator::Error),
    #[error("Contract Error\n{0}: {1}")]
    ContractInvoke(String, String),
    #[error(transparent)]
    StrKey(#[from] stellar_strkey::DecodeError),
    #[error(transparent)]
    ContractSpec(#[from] contract_spec::Error),
}

impl From<Infallible> for Error {
    fn from(_: Infallible) -> Self {
        unreachable!()
    }
}

impl Cmd {
    fn build_host_function_parameters(
        &self,
        contract_id: [u8; 32],
        spec_entries: &[ScSpecEntry],
    ) -> Result<(String, Spec, ScVec), Error> {
        let spec = Spec(Some(spec_entries.to_vec()));
        let mut cmd = clap::Command::new(self.contract_id.clone())
            .no_binary_name(true)
            .term_width(300)
            .max_term_width(300);

        for ScSpecFunctionV0 { name, .. } in spec.find_functions()? {
            cmd = cmd.subcommand(build_custom_cmd(&name.to_string_lossy(), &spec)?);
        }
        cmd.build();
        let mut matches_ = cmd.get_matches_from(&self.slop);
        let (function, matches_) = &matches_.remove_subcommand().unwrap();

        let func = spec.find_function(function)?;
        // create parsed_args in same order as the inputs to func
        let parsed_args = func
            .inputs
            .iter()
            .map(|i| {
                let name = i.name.to_string().unwrap();
                if let Some(mut val) = matches_.get_raw(&name) {
                    let mut s = val.next().unwrap().to_string_lossy().to_string();
                    if matches!(i.type_, ScSpecTypeDef::Address) {
                        let cmd = crate::commands::config::identity::address::Cmd {
                            name: Some(s.clone()),
                            hd_path: Some(0),
                            locator: self.config.locator.clone(),
                        };
                        if let Ok(address) = cmd.public_key() {
                            s = address.to_string();
                        }
                    }
                    spec.from_string(&s, &i.type_)
                        .map_err(|error| Error::CannotParseArg { arg: name, error })
                } else if matches!(i.type_, ScSpecTypeDef::Option(_)) {
                    Ok(ScVal::Void)
                } else {
                    Err(Error::MissingArgument(name))
                }
            })
            .collect::<Result<Vec<_>, Error>>()?;

        // Add the contract ID and the function name to the arguments
        let mut complete_args = vec![
            ScVal::Address(ScAddress::Contract(Hash(contract_id))),
            ScVal::Symbol(
                function
                    .try_into()
                    .map_err(|_| Error::FunctionNameTooLong(function.clone()))?,
            ),
        ];
        complete_args.extend_from_slice(parsed_args.as_slice());
        let complete_args_len = complete_args.len();

        Ok((
            function.clone(),
            spec,
            complete_args
                .try_into()
                .map_err(|_| Error::MaxNumberOfArgumentsReached {
                    current: complete_args_len,
                    maximum: ScVec::default().max_len(),
                })?,
        ))
    }

    pub async fn run(&self) -> Result<(), Error> {
        let res = self.invoke().await?;
        println!("{res}");
        Ok(())
    }

    pub async fn invoke(&self) -> Result<String, Error> {
        if self.config.is_no_network() {
            self.run_in_sandbox()
        } else {
            self.run_against_rpc_server().await
        }
    }

    pub async fn run_against_rpc_server(&self) -> Result<String, Error> {
        let network = self.config.get_network()?;
        tracing::trace!(?network);
        let contract_id = self.contract_id()?;
        let client = Client::new(&network.rpc_url)?;
        client
            .verify_network_passphrase(Some(&network.network_passphrase))
            .await?;
        let key = self.config.key_pair()?;

        // Get the account sequence number
        let public_strkey = stellar_strkey::ed25519::PublicKey(key.public.to_bytes()).to_string();
        let account_details = client.get_account(&public_strkey).await?;
        let sequence: i64 = account_details.seq_num.into();

        // Get the contract
        let spec_entries = if let Some(spec) = self.spec_entries()? {
            spec
        } else {
            client.get_remote_contract_spec(&contract_id).await?
        };

        // Get the ledger footprint
        let (function, spec, host_function_params) =
            self.build_host_function_parameters(contract_id, &spec_entries)?;
        let tx = build_invoke_contract_tx(
            host_function_params.clone(),
            sequence + 1,
            self.fee.fee,
            &key,
        )?;

        let (result, meta, events) = client
            .prepare_and_send_transaction(&tx, &key, &network.network_passphrase, Some(log_events))
            .await?;

        tracing::debug!(?result);
        if !events.is_empty() {
            tracing::debug!(?events);
        }

        let xdr::TransactionMeta::V3(xdr::TransactionMetaV3{soroban_meta: Some(xdr::SorobanTransactionMeta{return_value, ..}), ..}) = meta else {
            return Err(Error::MissingOperationResult);
        };

        output_to_string(&spec, &return_value, &function)
    }

    pub fn run_in_sandbox(&self) -> Result<String, Error> {
        let contract_id = self.contract_id()?;
        // Initialize storage and host
        // TODO: allow option to separate input and output file
        let mut state = self.config.get_state()?;

        // If a file is specified, deploy the contract to storage
        self.deploy_contract_in_sandbox(&mut state, &contract_id)?;

        // Create source account, adding it to the ledger if not already present.
        let source_account = AccountId(PublicKey::PublicKeyTypeEd25519(Uint256(
            self.config.key_pair()?.public.to_bytes(),
        )));
        let source_account_ledger_key = LedgerKey::Account(LedgerKeyAccount {
            account_id: source_account.clone(),
        });
        if !state
            .ledger_entries
            .iter()
            .any(|(k, _)| **k == source_account_ledger_key)
        {
            state.ledger_entries.push((
                Box::new(source_account_ledger_key),
                Box::new(default_account_ledger_entry(source_account.clone())),
            ));
        }

        let snap = Rc::new(state.clone());
        let mut storage = Storage::with_recording_footprint(snap);
        let spec_entries = if let Some(spec) = self.spec_entries()? {
            spec
        } else {
            utils::get_contract_spec_from_storage(&mut storage, &state.sequence_number, contract_id)
                .map_err(Error::CannotParseContractSpec)?
        };
        let budget = Budget::default();
        if self.unlimited_budget {
            budget.reset_unlimited();
        };
        let h = Host::with_storage_and_budget(storage, budget);
        h.switch_to_recording_auth();
        h.set_source_account(source_account);

        let mut ledger_info = state.ledger_info();
        ledger_info.sequence_number += 1;
        ledger_info.timestamp += 5;
        h.set_ledger_info(ledger_info.clone());

        let (function, spec, host_function_params) =
            self.build_host_function_parameters(contract_id, &spec_entries)?;
        h.set_diagnostic_level(DiagnosticLevel::Debug);
        let resv = h
            .invoke_function(HostFunction::InvokeContract(host_function_params))
            .map_err(|host_error| {
                if let Ok(error) = spec.find_error_type(host_error.error.get_code()) {
                    Error::ContractInvoke(error.name.to_string_lossy(), error.doc.to_string_lossy())
                } else {
                    host_error.into()
                }
            })?;

        let res_str = output_to_string(&spec, &resv, &function)?;

        state.update(&h);

        let contract_auth: Vec<SorobanAuthorizationEntry> = h
            .get_recorded_auth_payloads()?
            .into_iter()
            .map(|payload| SorobanAuthorizationEntry {
                credentials: match (payload.address, payload.nonce) {
                    (Some(address), Some(nonce)) => {
                        SorobanCredentials::Address(SorobanAddressCredentials {
                            address,
                            nonce,
                            signature_expiration_ledger: ledger_info.sequence_number + 1,
                            signature_args: ScVec::default(),
                        })
                    }
                    _ => SorobanCredentials::SourceAccount,
                },
                root_invocation: payload.invocation,
            })
            .collect();
        let (storage, budget, events, expiration_ledger_bumps) = h.try_finish().map_err(|h| h.1)?;
        let footprint = &create_ledger_footprint(&storage.footprint);
        log_events(
            footprint,
            &[contract_auth.try_into()?],
            &events.0,
            Some(&budget),
        );

        utils::bump_ledger_entry_expirations(&mut state.ledger_entries, &expiration_ledger_bumps);

        self.config.set_state(&mut state)?;
        if !events.0.is_empty() {
            self.events_file
                .commit(&events.0, &state, &self.config.locator.config_dir()?)?;
        }
        Ok(res_str)
    }

    pub fn deploy_contract_in_sandbox(
        &self,
        state: &mut soroban_ledger_snapshot::LedgerSnapshot,
        contract_id: &[u8; 32],
    ) -> Result<(), Error> {
        if let Some(contract) = self.read_wasm()? {
            let wasm_hash = utils::add_contract_code_to_ledger_entries(
                &mut state.ledger_entries,
                contract,
                state.min_persistent_entry_expiration,
            )
            .map_err(Error::CannotAddContractToLedgerEntries)?
            .0;
            utils::add_contract_to_ledger_entries(
                &mut state.ledger_entries,
                *contract_id,
                wasm_hash,
                state.min_persistent_entry_expiration,
            );
        }
        Ok(())
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
}

impl Cmd {
    fn contract_id(&self) -> Result<[u8; 32], Error> {
        utils::contract_id_from_str(&self.contract_id)
            .map_err(|e| Error::CannotParseContractId(self.contract_id.clone(), e))
    }
}

fn log_events(
    footprint: &LedgerFootprint,
    auth: &[VecM<SorobanAuthorizationEntry>],
    events: &[HostEvent],
    budget: Option<&Budget>,
) {
    crate::log::auth(auth);
    crate::log::events(events);
    crate::log::footprint(footprint);
    if let Some(budget) = budget {
        crate::log::budget(budget);
    }
}

pub fn output_to_string(spec: &Spec, res: &ScVal, function: &str) -> Result<String, Error> {
    let mut res_str = String::new();
    if let Some(output) = spec.find_function(function)?.outputs.get(0) {
        res_str = spec
            .xdr_to_json(res, output)
            .map_err(|e| Error::CannotPrintResult {
                result: res.clone(),
                error: e,
            })?
            .to_string();
    }
    Ok(res_str)
}

fn build_invoke_contract_tx(
    parameters: ScVec,
    sequence: i64,
    fee: u32,
    key: &ed25519_dalek::Keypair,
) -> Result<Transaction, Error> {
    let op = Operation {
        source_account: None,
        body: OperationBody::InvokeHostFunction(InvokeHostFunctionOp {
            host_function: HostFunction::InvokeContract(parameters),
            auth: VecM::default(),
        }),
    };
    Ok(Transaction {
        source_account: MuxedAccount::Ed25519(Uint256(key.public.to_bytes())),
        fee,
        seq_num: SequenceNumber(sequence),
        cond: Preconditions::None,
        memo: Memo::None,
        operations: vec![op].try_into()?,
        ext: TransactionExt::V0,
    })
}

fn build_custom_cmd(name: &str, spec: &Spec) -> Result<clap::Command, Error> {
    let func = spec
        .find_function(name)
        .map_err(|_| Error::FunctionNotFoundInContractSpec(name.to_string()))?;

    // Parse the function arguments
    let inputs_map = &func
        .inputs
        .iter()
        .map(|i| (i.name.to_string().unwrap(), i.type_.clone()))
        .collect::<HashMap<String, ScSpecTypeDef>>();
    let name: &'static str = Box::leak(name.to_string().into_boxed_str());
    let mut cmd = clap::Command::new(name)
        .no_binary_name(true)
        .term_width(300)
        .max_term_width(300);
    let kebab_name = name.to_kebab_case();
    if kebab_name != name {
        cmd = cmd.alias(kebab_name);
    }
    let func = spec.find_function(name).unwrap();
    let doc: &'static str = Box::leak(func.doc.to_string_lossy().into_boxed_str());
    cmd = cmd.about(Some(doc));
    for (name, type_) in inputs_map.iter() {
        let mut arg = clap::Arg::new(name);
        arg = arg
            .long(name)
            .alias(name.to_kebab_case())
            .num_args(1)
            .value_parser(clap::builder::NonEmptyStringValueParser::new())
            .long_help(spec.doc(name, type_).unwrap());

        if let Some(value_name) = spec.arg_value_name(type_, 0) {
            let value_name: &'static str = Box::leak(value_name.into_boxed_str());
            arg = arg.value_name(value_name);
        }

        // Set up special-case arg rules
        arg = match type_ {
            xdr::ScSpecTypeDef::Bool => arg.num_args(0).required(false),
            xdr::ScSpecTypeDef::Option(_val) => arg.required(false),
            xdr::ScSpecTypeDef::I256
            | xdr::ScSpecTypeDef::I128
            | xdr::ScSpecTypeDef::I64
            | xdr::ScSpecTypeDef::I32 => arg.allow_hyphen_values(true),
            _ => arg,
        };

        cmd = cmd.arg(arg);
    }
    Ok(cmd)
}
