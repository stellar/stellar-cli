use clap::Parser;
use std::{fmt::Debug, fs, io};
use stellar_contract_env_host::{Host, HostError, Vm};

use crate::contractid;

#[derive(Parser, Debug)]
pub struct Inspect {
    #[clap(long, parse(from_os_str))]
    file: std::path::PathBuf,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("io")]
    Io(#[from] io::Error),
    #[error("host")]
    Host(#[from] HostError),
}

impl Inspect {
    pub fn run(&self) -> Result<(), Error> {
        let contents = fs::read(&self.file)?;
        let h = Host::default();
        let vm = Vm::new(&h, contractid::ZERO, &contents)?;
        println!("Exports:");
        for f in vm.functions() {
            println!("- {}({})", f.name, "arg,".repeat(f.arity));
        }
        Ok(())
    }
}
