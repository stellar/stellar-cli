use std::{fmt::Debug, fs, io, rc::Rc};

use clap::Parser;
use stellar_contract_env_host::{
    budget::CostType,
    storage::Storage,
    xdr::{Error as XdrError, HostFunction, ScObject, ScVal, ScVec},
    Host, HostError, Vm,
};

use hex::{FromHex, FromHexError};

use crate::snapshot;

#[derive(Parser, Debug)]
pub struct Cmd {
    /// Name of function to invoke
    #[clap(long = "fn")]
    function: String,
    /// File to read and write ledger
    #[clap(long, parse(from_os_str), default_value("ledger.json"))]
    snapshot_file: std::path::PathBuf,
    /// Output the cost of the invocation to stderr
    #[clap(long = "cost")]
    cost: bool,
    #[clap(long, parse(from_os_str))]
    file: Option<std::path::PathBuf>,
    #[clap(long = "id", conflicts_with("file"))]
    contract_id: Option<String>,
    /// Argument to pass to the contract function
    #[clap(long = "arg", value_name = "arg", multiple = true)]
    args: Vec<String>,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("io")]
    Io(#[from] io::Error),
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

        let args = &self
            .args
            .iter()
            .map(|a| serde_json::from_str(a))
            .collect::<Result<Vec<ScVal>, serde_json::Error>>()?;

        if let Some(f) = &self.file {
            let contents = fs::read(f).unwrap();

            //TODO: contractID should be user specified
            let vm = Vm::new(&h, [0; 32].into(), &contents).unwrap();
            let res = vm.invoke_function(&h, &self.function, &ScVec(args.try_into()?))?;
            println!("{}", serde_json::to_string_pretty(&res)?);
        } else if let Some(id) = &self.contract_id {
            let contract_id: [u8; 32] = FromHex::from_hex(id)?;

            let mut complete_args = vec![
                ScVal::Object(Some(ScObject::Binary(contract_id.try_into()?))),
                ScVal::Symbol((&self.function).try_into()?),
            ];
            complete_args.extend_from_slice(args.as_slice());

            let res = h.invoke_function(HostFunction::Call, complete_args.try_into()?)?;
            println!("{}", serde_json::to_string_pretty(&res)?);
        } else {
            panic!("exactly one of file or contract_id need to be specified");
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
