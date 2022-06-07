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
        println!("File: {}", self.file.to_string_lossy());
        println!("Functions:");
        for f in vm.functions() {
            println!(
                " • {} ({}) -> ({})",
                f.name,
                vec!["val"; f.param_count].join(", "),
                vec!["res"; f.result_count].join(", ")
            );
        }
        if let Some(spec) = vm.custom_section("contractspecv0") {
            println!("Contract Spec: {}", base64::encode(spec));
        } else {
            println!("Contract Spec: None");
        }
        Ok(())
    }
}
