use std::{
    fmt::Debug,
    fs::{self, File},
    io,
    io::BufRead,
    io::Write,
    rc::Rc,
};

use clap::Parser;
use stellar_contract_env_host::{
    im_rc::OrdMap,
    storage::{SnapshotSource, Storage},
    xdr::{Error as XdrError, LedgerEntry, LedgerKey, ReadXdr, ScVal, ScVec, WriteXdr},
    Host, HostError, Vm,
};

use crate::strval::{self, StrValError};

#[derive(Parser, Debug)]
pub struct Invoke {
    #[clap(long, parse(from_os_str))]
    file: std::path::PathBuf,
    #[clap(long, parse(from_os_str))]
    db_file: std::path::PathBuf,
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
}

pub struct Snap {
    storage: OrdMap<LedgerKey, LedgerEntry>,
}

impl SnapshotSource for Snap {
    fn get(&self, key: &LedgerKey) -> Result<LedgerEntry, HostError> {
        match self.storage.get(key) {
            Some(v) => Ok(v.clone()),
            None => Err(HostError::General("Missing entry")),
        }
    }
    fn has(&self, key: &LedgerKey) -> Result<bool, HostError> {
        Ok(self.storage.contains_key(key))
    }
}

pub fn read_storage(
    input_file: &std::path::PathBuf,
) -> Result<OrdMap<LedgerKey, LedgerEntry>, Error> {
    let mut res = OrdMap::new();

    let file = match File::open(input_file) {
        Ok(f) => f,
        Err(e) => {
            //File doesn't exist, so treat this as an empty database and the file will be created later
            if e.kind() == std::io::ErrorKind::NotFound {
                return Ok(res);
            } else {
                return Err(Error::Io(e));
            }
        }
    };

    let mut lines = io::BufReader::new(file).lines();
    while let (Some(line1), Some(line2)) = (lines.next(), lines.next()) {
        let lk = LedgerKey::read_xdr(&mut line1?.as_bytes())?;
        let le = LedgerEntry::read_xdr(&mut line2?.as_bytes())?;
        res.insert(lk, le);
    }

    Ok(res)
}

pub fn commit_storage(
    startup_state: OrdMap<LedgerKey, LedgerEntry>,
    storage_map: &OrdMap<LedgerKey, Option<LedgerEntry>>,
    output_file: &std::path::PathBuf,
) -> Result<(), Error> {
    //Need to start off with the startup_state since it's possible the storage_map did not touch every existing entry
    let mut new_state = startup_state.clone();

    let mut file = File::create(output_file)?;
    for (lk, ole) in storage_map {
        if let Some(le) = ole {
            new_state.insert(lk.clone(), le.clone());
        } else {
            new_state.remove(lk);
        }
    }

    for (lk, le) in new_state {
        lk.write_xdr(&mut file)?;
        writeln!(&mut file)?;
        le.write_xdr(&mut file)?;
        writeln!(&mut file)?;
    }

    Ok(())
}

impl Invoke {
    pub fn run(&self) -> Result<(), Error> {
        let contents = fs::read(&self.file).unwrap();

        // Initialize storage and host
        // db_file format is one xdr object per line, where a LedgerKey is followed by the corresponding LedgerEntry.
        //
        // LedgerKey1
        // LedgerEntry1
        // LedgerKey2
        // LedgerEntry2
        // ...

        // TODO: allow option to separate input and output file
        let startup_state = read_storage(&self.db_file)?;
        let snap = Rc::new(Snap {
            storage: startup_state.clone(),
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

        commit_storage(startup_state, &storage.map, &self.db_file)?;
        Ok(())
    }
}
