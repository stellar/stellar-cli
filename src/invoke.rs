use std::{fmt::Debug, fs, io};

use clap::Parser;
use stellar_contract_env_host::{
    xdr::{Error as XdrError, ScVal, ScVec},
    Host, HostError, Vm,
};

use crate::{
    contractid,
    strval::{self, StrValError},
};

#[derive(Parser, Debug)]
pub struct Invoke {
    #[clap(long, parse(from_os_str))]
    file: std::path::PathBuf,
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

impl Invoke {
    pub fn run(&self) -> Result<(), Error> {
        let contents = fs::read(&self.file).unwrap();
        let h = Host::default();
        let vm = Vm::new(&h, contractid::ZERO, &contents).unwrap();
        let args = self
            .args
            .iter()
            .map(|a| strval::from_string(&h, a))
            .collect::<Result<Vec<ScVal>, StrValError>>()?;
        let res = vm.invoke_function(&h, &self.function, &ScVec(args.try_into()?))?;
        let res_str = strval::to_string(&h, res);
        println!("{}", res_str);
        Ok(())
    }
}
