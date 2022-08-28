use std::{fmt::Debug, fs, io, rc::Rc};

use clap::Parser;
use soroban_env_host::{
    budget::{Budget, CostType},
    storage::Storage,
    xdr::{
        Error as XdrError, HostFunction, ReadXdr, ScHostStorageErrorCode, ScObject, ScStatus, ScVal,
    },
    Host, HostError, Vm,
};

use hex::FromHexError;

use crate::contractspec;
use crate::snapshot;
use crate::strval::{self, StrValError};
use crate::utils;

#[derive(Parser, Debug)]
pub struct Cmd {
    /// Contract ID to invoke
    #[clap(long = "id")]
    contract_id: String,
    /// WASM file to deploy to the contract ID and invoke
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
    /// Output the cost execution to stderr
    #[clap(long = "cost")]
    cost: bool,
    /// File to persist ledger state
    #[clap(long, parse(from_os_str), default_value(".soroban/ledger.json"))]
    ledger_file: std::path::PathBuf,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("io")]
    Io(#[from] io::Error),
    #[error("strval")]
    StrVal(#[from] StrValError),
    #[error("xdr")]
    Xdr(#[from] XdrError),
    #[error("host")]
    Host(#[from] HostError),
    #[error("snapshot")]
    Snapshot(#[from] snapshot::Error),
    #[error("serde")]
    Serde(#[from] serde_json::Error),
    #[error("hex")]
    FromHex(#[from] FromHexError),
    #[error("contractnotfound")]
    FunctionNotFoundInContractSpec,
    #[error("toomanyarguments")]
    TooManyArguments,
}

#[derive(Clone, Debug)]
enum Arg {
    Arg(String),
    ArgXDR(String),
}

impl Cmd {
    pub fn run(&self, matches: &clap::ArgMatches) -> Result<(), Error> {
        let contract_id: [u8; 32] = utils::contract_id_from_str(&self.contract_id)?;

        // Initialize storage and host
        // TODO: allow option to separate input and output file
        let mut ledger_entries = snapshot::read(&self.ledger_file)?;

        //If a file is specified, deploy the contract to storage
        if let Some(f) = &self.wasm {
            let contract = fs::read(f).unwrap();
            utils::add_contract_to_ledger_entries(&mut ledger_entries, contract_id, contract)?;
        }

        let snap = Rc::new(snapshot::Snap {
            ledger_entries: ledger_entries.clone(),
        });
        let mut storage = Storage::with_recording_footprint(snap);
        let contents = utils::get_contract_wasm_from_storage(&mut storage, contract_id)?;
        let h = Host::with_storage_and_budget(storage, Budget::default());

        let vm = Vm::new(&h, contract_id.into(), &contents).unwrap();
        let inputs = match contractspec::function_spec(&vm, &self.function) {
            Some(s) => s.inputs,
            None => {
                return Err(Error::FunctionNotFoundInContractSpec);
            }
        };

        // re-assemble the args, to match the order given on the command line
        let indexed_args: Vec<(usize, Arg)> = matches
            .indices_of("args")
            .unwrap_or_default()
            .zip(self.args.iter())
            .map(|(a, b)| (a, Arg::Arg(b.to_string())))
            .collect();
        let indexed_args_xdr: Vec<(usize, Arg)> = matches
            .indices_of("args-xdr")
            .unwrap_or_default()
            .zip(self.args_xdr.iter())
            .map(|(a, b)| (a, Arg::ArgXDR(b.to_string())))
            .collect();
        let mut all_indexed_args: Vec<(usize, Arg)> = [indexed_args, indexed_args_xdr].concat();
        all_indexed_args.sort_by(|a, b| a.0.cmp(&b.0));

        if all_indexed_args.len() > inputs.len() {
            return Err(Error::TooManyArguments);
        }

        let parsed_args: Vec<ScVal> = all_indexed_args
            .iter()
            .zip(inputs.iter())
            .map(|(arg, input)| match &arg.1 {
                Arg::ArgXDR(s) => match base64::decode(s) {
                    Err(_) => Err(StrValError::InvalidValue),
                    Ok(b) => ScVal::from_xdr(b).map_err(StrValError::Xdr),
                },
                Arg::Arg(s) => strval::from_string(s, &input.type_),
            })
            .collect::<Result<Vec<_>, _>>()?;

        let mut complete_args = vec![
            ScVal::Object(Some(ScObject::Bytes(contract_id.try_into()?))),
            ScVal::Symbol((&self.function).try_into()?),
        ];
        complete_args.extend_from_slice(parsed_args.as_slice());

        let res = h.invoke_function(HostFunction::Call, complete_args.try_into()?)?;
        println!("{}", strval::to_string(&res)?);

        let (storage, budget, _) = h.try_finish().map_err(|_h| {
            HostError::from(ScStatus::HostStorageError(
                ScHostStorageErrorCode::UnknownError,
            ))
        })?;

        if self.cost {
            eprintln!("Cpu Insns: {}", budget.get_cpu_insns_count());
            eprintln!("Mem Bytes: {}", budget.get_mem_bytes_count());
            for cost_type in CostType::variants() {
                eprintln!("Cost ({:?}): {}", cost_type, budget.get_input(*cost_type));
            }
        }

        snapshot::commit(ledger_entries, &storage.map, &self.ledger_file)?;
        Ok(())
    }
}
