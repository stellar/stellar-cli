use std::collections::HashMap;
use std::convert::TryInto;
use std::ffi::OsString;
use std::num::ParseIntError;
use std::{fmt::Debug, fs, io, rc::Rc};

use clap::Parser;
use hex::FromHexError;
use soroban_env_host::xdr::ScSpecFunctionV0;
use soroban_env_host::{
    budget::{Budget, CostType},
    events::HostEvent,
    storage::Storage,
    xdr::{
        self, AccountId, AddressWithNonce, ContractAuth, ContractCodeEntry, ContractDataEntry,
        Error as XdrError, HostFunction, InvokeHostFunctionOp, InvokeHostFunctionResult,
        LedgerEntryData, LedgerFootprint, LedgerKey, LedgerKeyAccount, LedgerKeyContractCode,
        LedgerKeyContractData, Memo, MuxedAccount, Operation, OperationBody, OperationResult,
        OperationResultTr, Preconditions, PublicKey, ReadXdr, ScContractCode,
        ScHostStorageErrorCode, ScObject, ScSpecEntry, ScSpecTypeDef, ScStatic, ScStatus, ScVal,
        ScVec, SequenceNumber, Transaction, TransactionEnvelope, TransactionExt,
        TransactionResultResult, Uint256, VecM,
    },
    Host, HostError,
};
use soroban_spec::read::FromWasmError;

use super::super::{config, events, HEADING_SANDBOX};
use crate::{
    rpc::{self, Client},
    strval::{self, Spec},
    utils::{self, create_ledger_footprint, default_account_ledger_entry},
};

#[derive(Parser, Debug)]
#[allow(clippy::struct_excessive_bools)]
pub struct Cmd {
    /// Contract ID to invoke
    #[clap(long = "id")]
    contract_id: String,
    /// WASM file of the contract to invoke (if using sandbox will deploy this file)
    #[clap(long, parse(from_os_str))]
    wasm: Option<std::path::PathBuf>,
    /// Output the cost execution to stderr
    #[clap(long = "cost")]
    cost: bool,
    /// Run with an unlimited budget
    #[clap(long = "unlimited-budget", conflicts_with = "rpc-url")]
    unlimited_budget: bool,
    /// Output the footprint to stderr
    #[clap(long = "footprint")]
    footprint: bool,
    /// Output the contract auth for the transaction to stderr
    #[clap(long = "auth")]
    auth: bool,
    /// File to persist event output
    #[clap(
        long,
        parse(from_os_str),
        default_value(".soroban/events.json"),
        conflicts_with = "rpc-url",
        env = "SOROBAN_EVENTS_FILE",
        help_heading = HEADING_SANDBOX,
    )]
    events_file: std::path::PathBuf,

    // Arguments for contract as `--arg-name value`
    #[clap(last = true, name = "CONTRACT_FN_ARGS")]
    pub slop: Vec<OsString>,

    #[clap(flatten)]
    pub config: config::Args,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("parsing argument {arg}: {error}")]
    CannotParseArg { arg: String, error: strval::Error },
    #[error("cannot add contract to ledger entries: {0}")]
    CannotAddContractToLedgerEntries(XdrError),
    #[error(transparent)]
    // TODO: the Display impl of host errors is pretty user-unfriendly
    //       (it just calls Debug). I think we can do better than that
    Host(#[from] HostError),
    #[error("reading file {filepath}: {error}")]
    CannotReadContractFile {
        filepath: std::path::PathBuf,
        error: io::Error,
    },
    #[error("committing file {filepath}: {error}")]
    CannotCommitEventsFile {
        filepath: std::path::PathBuf,
        error: events::Error,
    },
    #[error("cannot parse contract ID {contract_id}: {error}")]
    CannotParseContractId {
        contract_id: String,
        error: FromHexError,
    },
    #[error("function {0} was not found in the contract")]
    FunctionNotFoundInContractSpec(String),
    #[error("parsing contract spec: {0}")]
    CannotParseContractSpec(FromWasmError),
    // #[error("unexpected number of arguments: {provided} (function {function} expects {expected} argument(s))")]
    // UnexpectedArgumentCount {
    //     provided: usize,
    //     expected: usize,
    //     function: String,
    // },
    #[error("function name {0} is too long")]
    FunctionNameTooLong(String),
    #[error("argument count ({current}) surpasses maximum allowed count ({maximum})")]
    MaxNumberOfArgumentsReached { current: usize, maximum: usize },
    #[error("cannot print result {result:?}: {error}")]
    CannotPrintResult { result: ScVal, error: strval::Error },
    #[error("xdr processing error: {0}")]
    Xdr(#[from] XdrError),
    #[error("error parsing int: {0}")]
    ParseIntError(#[from] ParseIntError),
    #[error(transparent)]
    Rpc(#[from] rpc::Error),
    #[error("unexpected contract code data type: {0:?}")]
    UnexpectedContractCodeDataType(LedgerEntryData),
    #[error("missing operation result")]
    MissingOperationResult,
    // #[error("args file error {0}")]
    // ArgsFile(std::path::PathBuf),
    #[error(transparent)]
    StrVal(#[from] strval::Error),
    #[error(transparent)]
    Config(#[from] config::Error),
    #[error("unexpected ({length}) simulate transaction result length")]
    UnexpectedSimulateTransactionResultSize { length: usize },
    #[error("Missing argument {0}")]
    MissingArgument(String),
}

impl Cmd {
    fn build_host_function_parameters(
        &self,
        contract_id: [u8; 32],
        spec_entries: &[ScSpecEntry],
    ) -> Result<(String, Spec, ScVec), Error> {
        let spec = Spec(Some(spec_entries.to_vec()));
        let mut cmd = clap::Command::new(&self.contract_id)
            .no_binary_name(true)
            .term_width(300)
            .max_term_width(300);

        for ScSpecFunctionV0 { name, .. } in spec.find_functions()? {
            let name: &'static str = Box::leak(name.to_string_lossy().into_boxed_str());
            // let doc: &'static str = Box::leak(doc.to_string_lossy().into_boxed_str());
            cmd = cmd.subcommand(build_custom_cmd(name, &spec)?);
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
                let s = match i.type_ {
                    ScSpecTypeDef::Bool => matches_.is_present(name.clone()).to_string(),
                    _ => matches_
                        .get_raw(&name)
                        .ok_or_else(|| Error::MissingArgument(name.clone()))?
                        .next()
                        .unwrap()
                        .to_string_lossy()
                        .to_string(),
                };
                spec.from_string(&s, &i.type_)
                    .map_err(|error| Error::CannotParseArg { arg: name, error })
            })
            .collect::<Result<Vec<_>, Error>>()?;

        // Add the contract ID and the function name to the arguments
        let mut complete_args = vec![
            ScVal::Object(Some(ScObject::Bytes(contract_id.try_into().unwrap()))),
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
        if self.config.is_no_network() {
            self.run_in_sandbox()
        } else {
            self.run_against_rpc_server().await
        }
    }

    async fn run_against_rpc_server(&self) -> Result<(), Error> {
        let contract_id = self.contract_id()?;
        let network = &self.config.get_network()?;
        let client = Client::new(&network.rpc_url);
        let key = self.config.key_pair()?;

        // Get the account sequence number
        let public_strkey = stellar_strkey::ed25519::PublicKey(key.public.to_bytes()).to_string();
        let account_details = client.get_account(&public_strkey).await?;
        // TODO: create a cmdline parameter for the fee instead of simply using the minimum fee
        let fee: u32 = 100;
        let sequence: i64 = account_details.seq_num.into();

        // Get the contract
        let spec_entries = if let Some(spec) = self.spec_entries()? {
            spec
        } else {
            // async closures are not yet stable
            get_remote_contract_spec_entries(&client, &contract_id).await?
        };

        // Get the ledger footprint
        let (function, spec, host_function_params) =
            self.build_host_function_parameters(contract_id, &spec_entries)?;
        let tx_without_footprint = build_invoke_contract_tx(
            host_function_params.clone(),
            None,
            None,
            sequence + 1,
            fee,
            &network.network_passphrase,
            &key,
        )?;
        let simulation_response = client.simulate_transaction(&tx_without_footprint).await?;
        if simulation_response.results.len() != 1 {
            return Err(Error::UnexpectedSimulateTransactionResultSize {
                length: simulation_response.results.len(),
            });
        }
        let result = &simulation_response.results[0];
        let footprint = LedgerFootprint::from_xdr_base64(&result.footprint)?;
        let auth = result
            .auth
            .iter()
            .map(ContractAuth::from_xdr_base64)
            .collect::<Result<Vec<_>, _>>()?;

        if self.footprint {
            eprintln!("Footprint: {}", serde_json::to_string(&footprint).unwrap());
        }

        if self.auth {
            eprintln!("Contract auth: {}", serde_json::to_string(&auth).unwrap());
        }

        // Send the final transaction with the actual footprint
        let tx = build_invoke_contract_tx(
            host_function_params,
            Some(footprint),
            Some(auth),
            sequence + 1,
            fee,
            &network.network_passphrase,
            &key,
        )?;

        let result = client.send_transaction(&tx).await?;
        let res = match result.result {
            TransactionResultResult::TxSuccess(ops) => {
                if ops.is_empty() {
                    return Err(Error::MissingOperationResult);
                }
                match &ops[0] {
                    OperationResult::OpInner(OperationResultTr::InvokeHostFunction(
                        InvokeHostFunctionResult::Success(r),
                    )) => r.clone(),
                    _ => return Err(Error::MissingOperationResult),
                }
            }
            _ => return Err(Error::MissingOperationResult),
        };
        let res_str = output_to_string(&spec, &res, &function)?;
        println!("{res_str}");
        // TODO: print cost

        Ok(())
    }

    fn run_in_sandbox(&self) -> Result<(), Error> {
        let contract_id = self.contract_id()?;
        // Initialize storage and host
        // TODO: allow option to separate input and output file
        let mut state = self.config.get_state()?;

        // If a file is specified, deploy the contract to storage
        self.deploy_contract_in_sandbox(&mut state, &contract_id)?;

        let key = self.config.key_pair()?;

        // Create source account, adding it to the ledger if not already present.
        let source_account = AccountId(PublicKey::PublicKeyTypeEd25519(Uint256(
            key.public.to_bytes(),
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
        let spec_entries = utils::get_contract_spec_from_storage(&mut storage, contract_id)
            .map_err(Error::CannotParseContractSpec)?;
        let h = Host::with_storage_and_budget(storage, Budget::default());
        if self.unlimited_budget {
            h.with_budget(|b| b.reset_unlimited());
        }
        h.switch_to_recording_auth();
        h.set_source_account(source_account);

        let mut ledger_info = state.ledger_info();
        ledger_info.sequence_number += 1;
        ledger_info.timestamp += 5;
        h.set_ledger_info(ledger_info);

        let (function, spec, host_function_params) =
            self.build_host_function_parameters(contract_id, &spec_entries)?;

        let res = h.invoke_function(HostFunction::InvokeContract(host_function_params))?;

        let res_str = output_to_string(&spec, &res, &function)?;

        println!("{res_str}");

        state.update(&h);

        let contract_auth: Vec<ContractAuth> = h
            .get_recorded_auth_payloads()?
            .into_iter()
            .map(|payload| {
                let address_with_nonce = match (payload.address, payload.nonce) {
                    (Some(address), Some(nonce)) => Some(AddressWithNonce { address, nonce }),
                    _ => None,
                };
                ContractAuth {
                    address_with_nonce,
                    root_invocation: payload.invocation,
                    signature_args: ScVec::default(),
                }
            })
            .collect();

        let (storage, budget, events) = h.try_finish().map_err(|_h| {
            HostError::from(ScStatus::HostStorageError(
                ScHostStorageErrorCode::UnknownError,
            ))
        })?;

        if self.footprint {
            eprintln!(
                "Footprint: {}",
                serde_json::to_string(&create_ledger_footprint(&storage.footprint)).unwrap(),
            );
        }

        if self.auth {
            eprintln!(
                "Contract auth: {}",
                serde_json::to_string(&contract_auth).unwrap(),
            );
        }

        if self.cost {
            eprintln!("Cpu Insns: {}", budget.get_cpu_insns_count());
            eprintln!("Mem Bytes: {}", budget.get_mem_bytes_count());
            for cost_type in CostType::variants() {
                eprintln!("Cost ({cost_type:?}): {}", budget.get_input(*cost_type));
            }
        }

        for (i, event) in events.0.iter().enumerate() {
            eprint!("#{i}: ");
            match event {
                HostEvent::Contract(e) => {
                    eprintln!("event: {}", serde_json::to_string(&e).unwrap());
                }
                HostEvent::Debug(e) => eprintln!("debug: {e}"),
            }
        }

        self.config.set_state(&mut state)?;

        events::commit(&events.0, &state, &self.events_file).map_err(|e| {
            Error::CannotCommitEventsFile {
                filepath: self.events_file.clone(),
                error: e,
            }
        })?;

        Ok(())
    }

    pub fn deploy_contract_in_sandbox(
        &self,
        state: &mut soroban_ledger_snapshot::LedgerSnapshot,
        contract_id: &[u8; 32],
    ) -> Result<(), Error> {
        if let Some(contract) = self.read_wasm()? {
            let wasm_hash =
                utils::add_contract_code_to_ledger_entries(&mut state.ledger_entries, contract)
                    .map_err(Error::CannotAddContractToLedgerEntries)?
                    .0;
            utils::add_contract_to_ledger_entries(
                &mut state.ledger_entries,
                *contract_id,
                wasm_hash,
            );
        }
        Ok(())
    }

    pub fn read_wasm(&self) -> Result<Option<Vec<u8>>, Error> {
        Ok(if let Some(wasm) = self.wasm.as_ref() {
            Some(fs::read(wasm).map_err(|e| Error::CannotReadContractFile {
                filepath: wasm.clone(),
                error: e,
            })?)
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
        utils::id_from_str(&self.contract_id).map_err(|e| Error::CannotParseContractId {
            contract_id: self.contract_id.clone(),
            error: e,
        })
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
    footprint: Option<LedgerFootprint>,
    auth: Option<Vec<ContractAuth>>,
    sequence: i64,
    fee: u32,
    network_passphrase: &str,
    key: &ed25519_dalek::Keypair,
) -> Result<TransactionEnvelope, Error> {
    // Use a default footprint if none provided
    let final_footprint = footprint.unwrap_or(LedgerFootprint {
        read_only: VecM::default(),
        read_write: VecM::default(),
    });
    let final_auth = auth.unwrap_or(Vec::default());
    let op = Operation {
        source_account: None,
        body: OperationBody::InvokeHostFunction(InvokeHostFunctionOp {
            function: HostFunction::InvokeContract(parameters),
            footprint: final_footprint,
            auth: final_auth.try_into()?,
        }),
    };
    let tx = Transaction {
        source_account: MuxedAccount::Ed25519(Uint256(key.public.to_bytes())),
        fee,
        seq_num: SequenceNumber(sequence),
        cond: Preconditions::None,
        memo: Memo::None,
        operations: vec![op].try_into()?,
        ext: TransactionExt::V0,
    };

    Ok(utils::sign_transaction(key, &tx, network_passphrase)?)
}

async fn get_remote_contract_spec_entries(
    client: &Client,
    contract_id: &[u8; 32],
) -> Result<Vec<ScSpecEntry>, Error> {
    // Get the contract from the network
    let contract_ref = client
        .get_ledger_entry(LedgerKey::ContractData(LedgerKeyContractData {
            contract_id: xdr::Hash(*contract_id),
            key: ScVal::Static(ScStatic::LedgerKeyContractCode),
        }))
        .await?;

    Ok(match LedgerEntryData::from_xdr_base64(contract_ref.xdr)? {
        LedgerEntryData::ContractData(ContractDataEntry {
            val: ScVal::Object(Some(ScObject::ContractCode(ScContractCode::WasmRef(hash)))),
            ..
        }) => {
            let contract_data = client
                .get_ledger_entry(LedgerKey::ContractCode(LedgerKeyContractCode { hash }))
                .await?;

            match LedgerEntryData::from_xdr_base64(contract_data.xdr)? {
                LedgerEntryData::ContractCode(ContractCodeEntry { code, .. }) => {
                    let code_vec: Vec<u8> = code.into();
                    soroban_spec::read::from_wasm(&code_vec)
                        .map_err(Error::CannotParseContractSpec)?
                }
                scval => return Err(Error::UnexpectedContractCodeDataType(scval)),
            }
        }
        LedgerEntryData::ContractData(ContractDataEntry {
            val: ScVal::Object(Some(ScObject::ContractCode(ScContractCode::Token))),
            ..
        }) => soroban_spec::read::parse_raw(&soroban_token_spec::spec_xdr())
            .map_err(FromWasmError::Parse)
            .map_err(Error::CannotParseContractSpec)?,
        scval => return Err(Error::UnexpectedContractCodeDataType(scval)),
    })
}

fn build_custom_cmd<'a>(name: &'a str, spec: &Spec) -> Result<clap::App<'a>, Error> {
    let func = spec
        .find_function(name)
        .map_err(|_| Error::FunctionNotFoundInContractSpec(name.to_string()))?;

    // Parse the function arguments
    let inputs_map = &func
        .inputs
        .iter()
        .map(|i| (i.name.to_string().unwrap(), i.type_.clone()))
        .collect::<HashMap<String, ScSpecTypeDef>>();
    let mut cmd = clap::Command::new(name)
        .no_binary_name(true)
        .term_width(300)
        .max_term_width(300);
    let func = spec.find_function(name).unwrap();
    let doc: &'static str = Box::leak(func.doc.to_string_lossy().into_boxed_str());
    cmd = cmd.about(Some(doc));
    for (name, type_) in inputs_map.iter() {
        let name: &'static str = Box::leak(name.clone().into_boxed_str());
        let mut arg = clap::Arg::new(name);
        arg = arg
            .long(name)
            .takes_value(true)
            .value_parser(clap::builder::NonEmptyStringValueParser::new())
            .long_help(spec.doc(name, type_).unwrap());

        if let Some(value_name) = spec.arg_value_name(type_, 0) {
            arg = arg.value_name(Box::leak(value_name.into_boxed_str()));
        }

        // Set up special-case arg rules
        arg = match type_ {
            xdr::ScSpecTypeDef::Bool => arg.takes_value(false).required(false),
            xdr::ScSpecTypeDef::Option(_val) => arg.required(false),
            xdr::ScSpecTypeDef::I128 | xdr::ScSpecTypeDef::I64 | xdr::ScSpecTypeDef::I32 => {
                arg.allow_hyphen_values(true)
            }
            _ => arg,
        };

        cmd = cmd.arg(arg);
    }
    Ok(cmd)
}
