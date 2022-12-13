use std::collections::HashMap;
use std::ffi::OsString;
use std::num::ParseIntError;
use std::path::PathBuf;
use std::{fmt::Debug, fs, io, rc::Rc};

use clap::Parser;
use hex::FromHexError;
use once_cell::sync::OnceCell;
use soroban_env_host::xdr::{
    self, ContractCodeEntry, ContractDataEntry, InvokeHostFunctionOp, LedgerEntryData,
    LedgerFootprint, LedgerKey, LedgerKeyAccount, LedgerKeyContractCode, LedgerKeyContractData,
    Memo, MuxedAccount, Operation, OperationBody, Preconditions, ScContractCode, ScSpecTypeDef,
    ScSpecTypeUdt, ScStatic, ScVec, SequenceNumber, Transaction, TransactionEnvelope,
    TransactionExt, VecM,
};
use soroban_env_host::{
    budget::{Budget, CostType},
    events::HostEvent,
    storage::Storage,
    xdr::{
        AccountId, Error as XdrError, HostFunction, PublicKey, ReadXdr, ScHostStorageErrorCode,
        ScObject, ScSpecEntry, ScStatus, ScVal, Uint256,
    },
    Host, HostError,
};
use soroban_spec::read::FromWasmError;
use stellar_strkey::StrkeyPublicKeyEd25519;

use crate::rpc::Client;
use crate::strval::Spec;
use crate::utils::{create_ledger_footprint, default_account_ledger_entry};
use crate::{rpc, snapshot, strval, utils};
use crate::{HEADING_RPC, HEADING_SANDBOX};

#[derive(Parser, Debug)]
pub struct Cmd {
    /// Contract ID to invoke
    #[clap(long = "id")]
    contract_id: String,
    /// WASM file of the contract to invoke (if using sandbox will deploy this file)
    #[clap(long, parse(from_os_str))]
    wasm: Option<std::path::PathBuf>,
    /// Function name to execute
    #[clap(long = "fn")]
    function: String,
    /// Argument to pass to the function
    #[clap(long = "arg", value_name = "arg", multiple = true)]
    args: Vec<String>,
    /// Argument to pass to the function (base64-encoded xdr)
    #[clap(long = "arg-xdr", value_name = "arg-xdr", multiple = true)]
    args_xdr: Vec<String>,
    /// File containing JSON Argument to pass to the function
    #[clap(long = "arg-file", value_name = "arg-file", multiple = true)]
    args_file: Vec<PathBuf>,
    /// File containing argument to pass to the function (base64-encoded xdr)
    #[clap(long = "arg-xdr-file", value_name = "arg-xdr-file", multiple = true)]
    args_xdr_file: Vec<PathBuf>,
    /// Output the cost execution to stderr
    #[clap(long = "cost")]
    cost: bool,
    /// Output the footprint to stderr
    #[clap(long = "footprint")]
    footprint: bool,

    /// Account ID to invoke as
    #[clap(
        long = "account",
        default_value = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF",
        conflicts_with = "rpc-url",
        help_heading = HEADING_SANDBOX,
    )]
    account_id: StrkeyPublicKeyEd25519,
    /// File to persist ledger state
    #[clap(
        long,
        parse(from_os_str),
        default_value(".soroban/ledger.json"),
        conflicts_with = "rpc-url",
        env = "SOROBAN_LEDGER_FILE",
        help_heading = HEADING_SANDBOX,
    )]
    ledger_file: std::path::PathBuf,

    /// Secret 'S' key used to sign the transaction sent to the rpc server
    #[clap(
        long = "secret-key",
        requires = "rpc-url",
        env = "SOROBAN_SECRET_KEY",
        help_heading = HEADING_RPC,
    )]
    secret_key: Option<String>,
    /// RPC server endpoint
    #[clap(
        long,
        conflicts_with = "account-id",
        requires = "secret-key",
        requires = "network-passphrase",
        env = "SOROBAN_RPC_URL",
        help_heading = HEADING_RPC,
    )]
    rpc_url: Option<String>,
    /// Network passphrase to sign the transaction sent to the rpc server
    #[clap(
        long = "network-passphrase",
        requires = "rpc-url",
        env = "SOROBAN_NETWORK_PASSPHRASE",
        help_heading = HEADING_RPC,
    )]
    network_passphrase: Option<String>,

    // Arguments for contract as `--arg-name value`, `--arg-xdr-name base64-encoded-xdr`
    #[clap(last = true, name = "ARGS")]
    pub slop: Vec<OsString>,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("parsing argument {arg}: {error}")]
    CannotParseArg { arg: String, error: strval::Error },
    // #[error("parsing XDR arg {arg}: {error}")]
    // CannotParseXdrArg { arg: String, error: XdrError },
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
    #[error("committing file {filepath}: {error}")]
    CannotCommitLedgerFile {
        filepath: std::path::PathBuf,
        error: snapshot::Error,
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
    #[error("cannot parse secret key")]
    CannotParseSecretKey,
    #[error(transparent)]
    Rpc(#[from] rpc::Error),
    #[error("unexpected contract code data type: {0:?}")]
    UnexpectedContractCodeDataType(LedgerEntryData),
    #[error("missing transaction result")]
    MissingTransactionResult,
    // #[error("args file error {0}")]
    // ArgsFile(std::path::PathBuf),
    #[error(transparent)]
    StrVal(#[from] strval::Error),
}

static INSTANCE: OnceCell<Vec<String>> = OnceCell::new();

impl Cmd {
    fn build_host_function_parameters(
        &self,
        contract_id: [u8; 32],
        spec_entries: &[ScSpecEntry],
    ) -> Result<ScVec, Error> {
        // Get the function spec from the contract code
        let spec = Spec(Some(spec_entries.to_vec()));
        let func = spec
            .find_function(&self.function)
            .map_err(|_| Error::FunctionNotFoundInContractSpec(self.function.clone()))?;

        // Parse the function arguments
        let inputs_map = &func
            .inputs
            .iter()
            .map(|i| (i.name.to_string().unwrap(), i.type_.clone()))
            .collect::<HashMap<String, ScSpecTypeDef>>();

        let cmd = build_custom_cmd(&self.function, inputs_map, &spec)?;
        let matches_ = cmd.get_matches_from(&self.slop);
        // let res = ;
        let parsed_args = inputs_map
            .iter()
            .map(|(name, t)| {
                let s = match t {
                    ScSpecTypeDef::Bool => matches_.is_present(name).to_string(),
                    _ => matches_
                        .get_raw(name)
                        .unwrap()
                        .next()
                        .unwrap()
                        .to_string_lossy()
                        .to_string(),
                };
                (s, t)
            })
            .map(|(s, t)| spec.from_string(&s, t))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| Error::CannotParseArg {
                arg: "Arg".to_string(),
                error,
            })?;

        // // Add the contract ID and the function name to the arguments
        let mut complete_args = vec![
            ScVal::Object(Some(ScObject::Bytes(contract_id.try_into().unwrap()))),
            ScVal::Symbol(
                (&self.function)
                    .try_into()
                    .map_err(|_| Error::FunctionNameTooLong(self.function.clone()))?,
            ),
        ];
        complete_args.extend_from_slice(parsed_args.as_slice());
        let complete_args_len = complete_args.len();

        complete_args
            .try_into()
            .map_err(|_| Error::MaxNumberOfArgumentsReached {
                current: complete_args_len,
                maximum: ScVec::default().max_len(),
            })
    }

    pub async fn run(&self) -> Result<(), Error> {
        if self.rpc_url.is_some() {
            self.run_against_rpc_server().await
        } else {
            self.run_in_sandbox()
        }
    }

    async fn run_against_rpc_server(&self) -> Result<(), Error> {
        let contract_id = self.contract_id()?;
        let client = Client::new(self.rpc_url.as_ref().unwrap());
        let key = utils::parse_secret_key(self.secret_key.as_ref().unwrap())
            .map_err(|_| Error::CannotParseSecretKey)?;

        // Get the account sequence number
        let public_strkey = StrkeyPublicKeyEd25519(key.public.to_bytes()).to_string();
        let account_details = client.get_account(&public_strkey).await?;
        // TODO: create a cmdline parameter for the fee instead of simply using the minimum fee
        let fee: u32 = 100;
        let sequence = account_details.sequence.parse::<i64>()?;

        // Get the contract
        let spec_entries = if let Some(spec) = self.spec_entries()? {
            spec
        } else {
            // async closures are not yet stable
            get_remote_contract_spec_entries(&client, &contract_id).await?
        };

        // Get the ledger footprint
        let host_function_params =
            self.build_host_function_parameters(contract_id, &spec_entries)?;
        let tx_without_footprint = build_invoke_contract_tx(
            host_function_params.clone(),
            None,
            sequence + 1,
            fee,
            self.network_passphrase.as_ref().unwrap(),
            &key,
        )?;
        let simulation_response = client.simulate_transaction(&tx_without_footprint).await?;
        let footprint = LedgerFootprint::from_xdr_base64(simulation_response.footprint)?;

        if self.footprint {
            eprintln!("Footprint: {}", serde_json::to_string(&footprint).unwrap(),);
        }

        // Send the final transaction with the actual footprint
        let tx = build_invoke_contract_tx(
            host_function_params,
            Some(footprint),
            sequence + 1,
            fee,
            self.network_passphrase.as_ref().unwrap(),
            &key,
        )?;

        let results = client.send_transaction(&tx).await?;
        if results.is_empty() {
            return Err(Error::MissingTransactionResult);
        }
        let res = ScVal::from_xdr_base64(&results[0].xdr)?;
        let res_str = strval::to_string(&res).map_err(|e| Error::CannotPrintResult {
            result: res,
            error: e,
        })?;

        println!("{res_str}");
        // TODO: print cost

        Ok(())
    }

    fn run_in_sandbox(&self) -> Result<(), Error> {
        let contract_id = self.contract_id()?;
        // Initialize storage and host
        // TODO: allow option to separate input and output file
        let mut state =
            snapshot::read(&self.ledger_file).map_err(|e| Error::CannotReadLedgerFile {
                filepath: self.ledger_file.clone(),
                error: e,
            })?;

        // If a file is specified, deploy the contract to storage
        if let Some(contract) = self.read_wasm()? {
            let wasm_hash = utils::add_contract_code_to_ledger_entries(&mut state.1, contract)
                .map_err(Error::CannotAddContractToLedgerEntries)?
                .0;
            utils::add_contract_to_ledger_entries(&mut state.1, contract_id, wasm_hash);
        }

        // Create source account, adding it to the ledger if not already present.
        let source_account = AccountId(PublicKey::PublicKeyTypeEd25519(Uint256(self.account_id.0)));
        let source_account_ledger_key = LedgerKey::Account(LedgerKeyAccount {
            account_id: source_account.clone(),
        });
        if !state.1.contains_key(&source_account_ledger_key) {
            state.1.insert(
                source_account_ledger_key,
                default_account_ledger_entry(source_account.clone()),
            );
        }

        let snap = Rc::new(snapshot::Snap {
            ledger_entries: state.1.clone(),
        });
        let mut storage = Storage::with_recording_footprint(snap);
        let spec_entries = utils::get_contract_spec_from_storage(&mut storage, contract_id)
            .map_err(Error::CannotParseContractSpec)?;
        let h = Host::with_storage_and_budget(storage, Budget::default());
        h.set_source_account(source_account);

        let mut ledger_info = state.0.clone();
        ledger_info.sequence_number += 1;
        ledger_info.timestamp += 5;
        h.set_ledger_info(ledger_info.clone());

        let host_function_params =
            self.build_host_function_parameters(contract_id, &spec_entries)?;

        let res = h.invoke_function(HostFunction::InvokeContract(host_function_params))?;
        let res_str = strval::to_string(&res).map_err(|e| Error::CannotPrintResult {
            result: res,
            error: e,
        })?;

        println!("{res_str}");

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

        snapshot::commit(state.1, ledger_info, &storage.map, &self.ledger_file).map_err(|e| {
            Error::CannotCommitLedgerFile {
                filepath: self.ledger_file.clone(),
                error: e,
            }
        })?;
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

fn build_invoke_contract_tx(
    parameters: ScVec,
    footprint: Option<LedgerFootprint>,
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
    let op = Operation {
        source_account: None,
        body: OperationBody::InvokeHostFunction(InvokeHostFunctionOp {
            function: HostFunction::InvokeContract(parameters),
            footprint: final_footprint,
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
                    soroban_spec::read::from_wasm(&code).map_err(Error::CannotParseContractSpec)?
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

fn build_custom_cmd<'a>(
    name: &'a str,
    inputs_map: &'a HashMap<String, ScSpecTypeDef>,
    spec: &Spec,
) -> Result<clap::App<'a>, Error> {
    // Todo make new error
    INSTANCE
        .set(inputs_map.keys().map(Clone::clone).collect::<Vec<String>>())
        .unwrap();

    let names: &'static [String] = INSTANCE.get().unwrap();
    let mut cmd = clap::Command::new(name).no_binary_name(true);

    for (i, type_) in inputs_map.values().enumerate() {
        let name = names[i].as_str();
        let mut arg = clap::Arg::new(name);
        arg = arg
            .long(name)
            .takes_value(true)
            .value_parser(clap::builder::NonEmptyStringValueParser::new());

        arg = match type_ {
            xdr::ScSpecTypeDef::Val => todo!(),
            xdr::ScSpecTypeDef::U64 => arg
                .value_name("u64")
                .value_parser(clap::builder::RangedU64ValueParser::<u64>::new()),
            xdr::ScSpecTypeDef::I64 => arg
                .value_name("i64")
                .value_parser(clap::builder::RangedI64ValueParser::<i64>::new()),
            xdr::ScSpecTypeDef::U128 => todo!(),
            xdr::ScSpecTypeDef::I128 => todo!(),
            xdr::ScSpecTypeDef::U32 => arg
                .value_name("u32")
                .value_parser(clap::builder::RangedU64ValueParser::<u32>::new()),
            xdr::ScSpecTypeDef::I32 => arg
                .value_name("i32")
                .value_parser(clap::builder::RangedU64ValueParser::<i32>::new()),
            xdr::ScSpecTypeDef::Bool => arg.takes_value(false).required(false),
            xdr::ScSpecTypeDef::Symbol => arg.value_name("symbol"),
            xdr::ScSpecTypeDef::Bitset => todo!(),
            xdr::ScSpecTypeDef::Status => todo!(),
            xdr::ScSpecTypeDef::Bytes => arg.value_name("bytes"),
            xdr::ScSpecTypeDef::Invoker => todo!(),
            xdr::ScSpecTypeDef::AccountId => arg
                .value_name("AccountId")
                .next_line_help(true)
                .help("ed25519 Public Key"),
            xdr::ScSpecTypeDef::Option(_val) => arg.required(false),
            xdr::ScSpecTypeDef::Result(_) => todo!(),
            xdr::ScSpecTypeDef::Vec(_) => todo!(),
            xdr::ScSpecTypeDef::Map(map) => todo!("{map:#?}"),
            xdr::ScSpecTypeDef::Set(_) => todo!(),
            xdr::ScSpecTypeDef::Tuple(_) => todo!(),
            xdr::ScSpecTypeDef::BytesN(_) => todo!(),
            xdr::ScSpecTypeDef::Udt(ScSpecTypeUdt { name }) => {
                match spec.find(&name.to_string_lossy())? {
                    ScSpecEntry::FunctionV0(_) => todo!(),
                    ScSpecEntry::UdtStructV0(_) => arg.value_name("struct"),
                    ScSpecEntry::UdtUnionV0(_) => arg.value_name("enum"),
                    ScSpecEntry::UdtEnumV0(_) => arg
                        .value_name("u32")
                        .value_parser(clap::builder::RangedU64ValueParser::<u32>::new()),
                    ScSpecEntry::UdtErrorEnumV0(_) => todo!(),
                }
            }
        };
        cmd = cmd.arg(arg);
    }
    cmd.build();
    Ok(cmd)
}

#[cfg(test)]
mod test {

    use super::*;
    use crate::strval;
    use serde_json::json;
    use soroban_env_host::xdr::{ScSpecTypeDef, ScSpecTypeUdt};

    #[test]
    fn parse_bool() {
        println!(
            "{:#?}",
            strval::from_string_primitive("true", &ScSpecTypeDef::Bool,).unwrap()
        );
    }

    #[test]
    fn parse_u32() {
        let u32_ = 42u32;
        let res = &format!("{u32_}");
        println!(
            "{:#?}",
            strval::from_string_primitive(res, &ScSpecTypeDef::U32,).unwrap()
        );
    }

    #[test]
    fn parse_i32() {
        let i32_ = -42_i32;
        let res = &format!("{i32_}");
        println!(
            "{:#?}",
            strval::from_string_primitive(res, &ScSpecTypeDef::I32,).unwrap()
        );
    }

    #[test]
    fn parse_u64() {
        let b = 42_000_000_000u64;
        let res = &format!("{b}");
        println!(
            "{:#?}",
            strval::from_string_primitive(res, &ScSpecTypeDef::U64,).unwrap()
        );
    }

    #[test]
    fn parse_symbol() {
        // let b = "hello";
        // let res = &parse_json(&HashMap::new(), &ScSpecTypeDef::Symbol, &json! {b}).unwrap();
        // println!("{res}");
        println!(
            "{:#?}",
            strval::from_string_primitive(r#""hello""#, &ScSpecTypeDef::Symbol).unwrap()
        );
    }

    #[test]
    fn parse_symbol_with_no_quotation_marks() {
        // let b = "hello";
        // let res = &parse_json(&HashMap::new(), &ScSpecTypeDef::Symbol, &json! {b}).unwrap();
        // println!("{res}");
        println!(
            "{:#?}",
            strval::from_string_primitive("hello", &ScSpecTypeDef::Symbol).unwrap()
        );
    }

    #[test]
    fn parse_obj() {
        let type_ = &ScSpecTypeDef::Udt(ScSpecTypeUdt {
            name: "Test".parse().unwrap(),
        });
        let entries = get_spec();
        let val = &json!({"a": 42, "b": false, "c": "world"});
        println!("{:#?}", entries.from_json(val, type_));
    }

    #[test]
    fn parse_enum() {
        let entries = get_spec();
        let func = entries.find_function("enum_2_str").unwrap();
        println!("{func:#?}");
        let type_ = &func.inputs.as_slice()[0].type_;
        println!("{:#?}", entries.from_json(&json!("First"), type_));
    }

    #[test]
    fn parse_enum_const() {
        let entries = get_spec();
        let func = entries.find_function("card").unwrap();
        println!("{func:#?}");
        let type_ = &func.inputs.as_slice()[0].type_;
        println!("{:#?}", entries.from_json(&json!(11), type_));
    }

    fn get_spec() -> Spec {
        let res = soroban_spec::read::from_wasm(
            &fs::read("../../target/wasm32-unknown-unknown/test-wasms/test_custom_types.wasm")
                .unwrap(),
        )
        .unwrap();
        Spec(Some(res))
    }
}
