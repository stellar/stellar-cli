use std::{fmt::Debug, fs, rc::Rc};

use clap::Parser;
use soroban_env_host::{
    budget::{Budget, CostType},
    events::HostEvent,
    storage::Storage,
    xdr::{
        HostFunction, ReadXdr, ScHostStorageErrorCode, ScObject, ScSpecFunctionInputV0, ScStatus,
        ScVal, VecM,
    },
    Host, HostError, Vm,
};

use crate::contractspec;
use crate::error;
use crate::snapshot;
use crate::strval;
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

#[derive(Clone, Debug)]
enum Arg {
    Arg(String),
    ArgXdr(String),
}

impl Cmd {
    fn parse_args(
        &self,
        matches: &clap::ArgMatches,
        inputs: &VecM<ScSpecFunctionInputV0, 10>,
    ) -> Result<Vec<ScVal>, error::Cmd> {
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
            .map(|(a, b)| (a, Arg::ArgXdr(b.to_string())))
            .collect();
        let mut all_indexed_args: Vec<(usize, Arg)> = [indexed_args, indexed_args_xdr].concat();
        all_indexed_args.sort_by(|a, b| a.0.cmp(&b.0));

        if all_indexed_args.len() != inputs.len() {
            return Err(error::Cmd::UnexpectedArgumentCount {
                provided: all_indexed_args.len(),
                expected: inputs.len(),
                function: self.function.clone(),
            });
        }

        all_indexed_args
            .iter()
            .zip(inputs.iter())
            .map(|(arg, input)| match &arg.1 {
                Arg::ArgXdr(s) => ScVal::from_xdr_base64(s.to_string()).map_err(|e| {
                    error::Cmd::CannotParseXdrArg {
                        arg: s.clone(),
                        error: e,
                    }
                }),
                Arg::Arg(s) => {
                    strval::from_string(s, &input.type_).map_err(|e| error::Cmd::CannotParseArg {
                        arg: s.clone(),
                        error: e,
                    })
                }
            })
            .collect::<Result<Vec<_>, _>>()
    }

    pub fn run(&self, matches: &clap::ArgMatches) -> Result<(), error::Cmd> {
        let contract_id: [u8; 32] =
            utils::contract_id_from_str(&self.contract_id).map_err(|e| {
                error::Cmd::CannotParseContractId {
                    contract_id: self.contract_id.clone(),
                    error: e,
                }
            })?;

        // Initialize storage and host
        // TODO: allow option to separate input and output file
        let mut ledger_entries =
            snapshot::read(&self.ledger_file).map_err(|e| error::Cmd::CannotReadLedgerFile {
                filepath: self.ledger_file.clone(),
                error: e,
            })?;

        //If a file is specified, deploy the contract to storage
        if let Some(f) = &self.wasm {
            let contract = fs::read(f).map_err(|e| error::Cmd::CannotReadContractFile {
                filepath: f.clone(),
                error: e,
            })?;
            utils::add_contract_to_ledger_entries(&mut ledger_entries, contract_id, contract)
                .map_err(error::Cmd::CannotAddContractToLedgerEntries)?;
        }

        let snap = Rc::new(snapshot::Snap {
            ledger_entries: ledger_entries.clone(),
        });
        let mut storage = Storage::with_recording_footprint(snap);
        let contents = utils::get_contract_wasm_from_storage(&mut storage, contract_id)?;
        let h = Host::with_storage_and_budget(storage, Budget::default());

        let vm = Vm::new(&h, contract_id.into(), &contents)?;
        let inputs = match contractspec::function_spec(&vm, &self.function) {
            Some(s) => s.inputs,
            None => {
                return Err(error::Cmd::FunctionNotFoundInContractSpec(
                    self.function.clone(),
                ));
            }
        };

        let parsed_args = self.parse_args(matches, &inputs)?;

        let mut complete_args = vec![
            ScVal::Object(Some(ScObject::Bytes(contract_id.try_into().unwrap()))),
            ScVal::Symbol(
                (&self.function)
                    .try_into()
                    .map_err(|_| error::Cmd::FunctionNameTooLong(self.function.clone()))?,
            ),
        ];
        complete_args.extend_from_slice(parsed_args.as_slice());
        let complete_args_len = complete_args.len();

        let final_args =
            complete_args
                .try_into()
                .map_err(|_| error::Cmd::MaxNumberOfArgumentsReached {
                    current: complete_args_len,
                    maximum: soroban_env_host::xdr::ScVec::default().max_len(),
                })?;
        let res = h.invoke_function(HostFunction::Call, final_args)?;
        let res_str = strval::to_string(&res).map_err(|e| error::Cmd::CannotPrintResult {
            result: res,
            error: e,
        })?;
        println!("{}", res_str);

        let (storage, budget, events) = h.try_finish().map_err(|_h| {
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

        for (i, event) in events.0.iter().enumerate() {
            eprintln!("Event #{}:", i);
            match event {
                HostEvent::Contract(e) => eprint!("{}", serde_json::to_string(&e).unwrap()),
                HostEvent::Debug(e) => eprint!("{}", e),
            }
        }

        snapshot::commit(ledger_entries, &storage.map, &self.ledger_file).map_err(|e| {
            error::Cmd::CannotCommitLedgerFile {
                filepath: self.ledger_file.clone(),
                error: e,
            }
        })?;
        Ok(())
    }
}
