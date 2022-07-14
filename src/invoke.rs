use std::{
    fmt::Debug,
    fs::{self},
    io,
    rc::Rc,
};

use clap::Parser;
use stellar_contract_env_host::{
    storage::Storage,
    xdr::{Error as XdrError, ScVal, ScVec},
    Host, HostError, Vm,
};

use crate::strval::{self, StrValError};

#[derive(Parser, Debug)]
pub struct Invoke {
    #[clap(long, parse(from_os_str))]
    file: std::path::PathBuf,
    #[clap(long, parse(from_os_str), default_value("ledger.json"))]
    snapshot_file: std::path::PathBuf,
    #[clap(long = "fn")]
    function: String,
    #[clap(long = "arg", multiple_occurrences = true)]
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

impl Invoke {
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
        let args = self
            .args
            .iter()
            .map(|a| strval::from_string(&h, a))
            .collect::<Result<Vec<ScVal>, StrValError>>()?;
        let res = vm.invoke_function(&h, &self.function, &ScVec(args.try_into()?))?;
        let res_str = strval::to_string(&h, res);
        println!("{}", res_str);

        let storage = h
            .recover_storage()
            .map_err(|_h| HostError::General("could not get storage from host"))?;

        snapshot::commit(ledger_entries, &storage.map, &self.snapshot_file)?;
        Ok(())
    }
}
