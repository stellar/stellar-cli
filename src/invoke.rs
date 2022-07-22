use std::{
    fmt::Debug,
    fs::{self},
    io,
    io::Cursor,
    rc::Rc,
};

use clap::Parser;
use stellar_contract_env_host::{
    budget::CostType,
    storage::Storage,
    xdr::{Error as XdrError, ReadXdr, ScVal, ScVec, ScSpecEntry, ScSpecFunctionV0},
    Host, HostError, Vm,
};

use crate::strval::{self, StrValError};

#[derive(Parser, Debug)]
pub struct Cmd {
    /// WASM file containing contract
    #[clap(long, parse(from_os_str))]
    file: std::path::PathBuf,
    /// File to read and write ledger
    #[clap(long, parse(from_os_str), default_value("ledger.json"))]
    snapshot_file: std::path::PathBuf,
    /// Output the cost of the invocation to stderr
    #[clap(long = "cost")]
    cost: bool,
    /// Name of function to invoke
    #[clap(long = "fn")]
    function: String,
    /// Argument to pass to the contract function
    #[clap(long = "arg", value_name = "arg", multiple = true)]
    args: Vec<String>,
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
    #[error("serde")]
    Serde(#[from] serde_json::Error),
}

pub mod snapshot {
    use std::fs::File;

    use super::{Error, HostError};
    use stellar_contract_env_host::{
        im_rc::OrdMap,
        storage::SnapshotSource,
        xdr::{LedgerEntry, LedgerKey, VecM},
    };

    pub struct Snap {
        pub ledger_entries: OrdMap<LedgerKey, LedgerEntry>,
    }

    impl SnapshotSource for Snap {
        fn get(&self, key: &LedgerKey) -> Result<LedgerEntry, HostError> {
            match self.ledger_entries.get(key) {
                Some(v) => Ok(v.clone()),
                None => Err(HostError::General("missing entry")),
            }
        }
        fn has(&self, key: &LedgerKey) -> Result<bool, HostError> {
            Ok(self.ledger_entries.contains_key(key))
        }
    }

    // snapshot_file format is the default serde JSON representation of VecM<(LedgerKey, LedgerEntry)>
    pub fn read(input_file: &std::path::PathBuf) -> Result<OrdMap<LedgerKey, LedgerEntry>, Error> {
        let mut res = OrdMap::new();

        let mut file = match File::open(input_file) {
            Ok(f) => f,
            Err(e) => {
                //File doesn't exist, so treat this as an empty database and the file will be created later
                if e.kind() == std::io::ErrorKind::NotFound {
                    return Ok(res);
                }
                return Err(Error::Io(e));
            }
        };

        let state: VecM<(LedgerKey, LedgerEntry)> = serde_json::from_reader(&mut file)?;
        res = state.iter().cloned().collect();

        Ok(res)
    }

    pub fn commit(
        mut new_state: OrdMap<LedgerKey, LedgerEntry>,
        storage_map: &OrdMap<LedgerKey, Option<LedgerEntry>>,
        output_file: &std::path::PathBuf,
    ) -> Result<(), Error> {
        //Need to start off with the existing snapshot (new_state) since it's possible the storage_map did not touch every existing entry
        let file = File::create(output_file)?;
        for (lk, ole) in storage_map {
            if let Some(le) = ole {
                new_state.insert(lk.clone(), le.clone());
            } else {
                new_state.remove(lk);
            }
        }

        let vec_new_state: VecM<(LedgerKey, LedgerEntry)> =
            new_state.into_iter().collect::<Vec<_>>().try_into()?;
        serde_json::to_writer(&file, &vec_new_state)?;

        Ok(())
    }
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        let contents = fs::read(&self.file).unwrap();

        // Initialize storage and host
        // TODO: allow option to separate input and output file
        let ledger_entries = snapshot::read(&self.snapshot_file)?;
        let snap = Rc::new(snapshot::Snap {
            ledger_entries: ledger_entries.clone(),
        });
        let storage = Storage::with_recording_footprint(snap);

        let h = Host::with_storage(storage);

        //TODO: contractID should be user specified
        let vm = Vm::new(&h, [0; 32].into(), &contents).unwrap();
        let input_types = match Self::function_spec(&vm, &self.function) {
            Some(s) => { s.input_types },
            None => {
                // TODO: Better error here
                return Err(Error::StrVal(StrValError::UnknownError));
            }
        };
        let args = self
            .args
            .iter()
            .zip(input_types.iter())
            .map(|(a, t)| strval::from_string(a, t))
            .collect::<Result<Vec<ScVal>, StrValError>>()?;
        let res = vm.invoke_function(&h, &self.function, &ScVec(args.try_into()?))?;
        let res_str = strval::to_string(&h, res);
        println!("{}", res_str);

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

        snapshot::commit(ledger_entries, &storage.map, &self.snapshot_file)?;
        Ok(())
    }

    fn function_spec(vm: &Rc<Vm>, name: &str) -> Option<ScSpecFunctionV0> {
        let spec = vm.custom_section("contractspecv0")?;
        let mut cursor = Cursor::new(spec);
        for spec_entry in ScSpecEntry::read_xdr_iter(&mut cursor) {
            match spec_entry {
                Ok(ScSpecEntry::FunctionV0(f)) =>  {
                    if let Ok(n) = f.name.to_string() {
                        if n == name {
                            return Some(f);
                        }
                    }
                }
                _ => {}
            }
        }
        return None;
    }
}
