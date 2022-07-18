use std::{
    fmt::Debug,
    fs::{self, File},
    io,
    rc::Rc,
};

use clap::{Parser, Subcommand};
use stellar_contract_env_host::{
    budget::CostType,
    storage::Storage,
    xdr::{Error as XdrError, HostFunction, ScVal, ScVec},
    Host, HostError, Vm,
};

use crate::strval::{self, StrValError};

use crate::snapshot;

#[derive(Subcommand, Debug)]
enum Commands {
    Call,
    VmFn {
        /// Name of function to invoke
        #[clap(long = "fn")]
        function: String,
        /// Argument to pass to the contract function
        #[clap(long = "arg", value_name = "arg", multiple = true)]
        args: Vec<String>,
    },
}

#[derive(Parser, Debug)]
pub struct Cmd {
    //TODO: move this into each subcommand instead?
    /// If command == vm-fn, then this is the WASM file containing contract, otherwise, it's a JSON SCVec
    #[clap(long, parse(from_os_str))]
    file: std::path::PathBuf,
    /// File to read and write ledger
    #[clap(long, parse(from_os_str), default_value("ledger.json"))]
    snapshot_file: std::path::PathBuf,
    /// Output the cost of the invocation to stderr
    #[clap(long = "cost")]
    cost: bool,
    #[clap(subcommand)]
    command: Commands,
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
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        // Initialize storage and host
        // TODO: allow option to separate input and output file
        let ledger_entries = snapshot::read(&self.snapshot_file)?;
        let snap = Rc::new(snapshot::Snap {
            ledger_entries: ledger_entries.clone(),
        });
        let storage = Storage::with_recording_footprint(snap);

        let mut h = Host::with_storage(storage);

        match &self.command {
            Commands::Call => {
                let mut file = File::open(&self.file).unwrap();
                let args: ScVec = serde_json::from_reader(&mut file)?;
                let _res = h.invoke_function(HostFunction::Call, args)?;
            }
            Commands::VmFn { function, args } => {
                let contents = fs::read(&self.file).unwrap();
                let args = args
                    .iter()
                    .map(|a| strval::from_string(&h, a))
                    .collect::<Result<Vec<ScVal>, StrValError>>()?;

                //TODO: contractID should be user specified
                let vm = Vm::new(&h, [0; 32].into(), &contents).unwrap();
                let res = vm.invoke_function(&h, function, &ScVec(args.try_into()?))?;
                let res_str = strval::to_string(&h, res);
                println!("{}", res_str);
            }
        }

        if self.cost {
            h.get_budget(|b| {
                eprintln!("Cpu Insns: {}", b.cpu_insns.get_count());
                eprintln!("Mem Bytes: {}", b.mem_bytes.get_count());
                for cost_type in CostType::variants() {
                    eprintln!("Cost ({:?}): {}", cost_type, b.get_input(*cost_type));
                }
            });
        }

        let storage = h
            .recover_storage()
            .map_err(|_h| HostError::General("could not get storage from host"))?;

        snapshot::commit(ledger_entries, Some(&storage.map), &self.snapshot_file)?;
        Ok(())
    }
}
