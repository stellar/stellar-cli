use std::{error, fmt::Debug, fmt::Display, fs};

use clap::Parser;
use stellar_contract_env_host::{
    xdr::{Error as XdrError, ScVal, ScVec},
    Host, Vm,
};

use crate::strval::{self, StrValError};

#[derive(Parser, Debug)]
pub struct Invoke {
    #[clap(long, parse(from_os_str))]
    file: std::path::PathBuf,
    #[clap(long = "fn")]
    function: String,
    #[clap(long = "arg", multiple_occurrences = true)]
    args: Vec<String>,
}

#[derive(Debug)]
pub enum Error {
    Other(Box<dyn error::Error>),
    StrVal(StrValError),
    Xdr(XdrError),
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Other(e) => e.source(),
            Self::StrVal(e) => e.source(),
            Self::Xdr(e) => e.source(),
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invoke error: ")?;
        match self {
            Self::Other(e) => std::fmt::Display::fmt(&e, f)?,
            Self::StrVal(e) => std::fmt::Display::fmt(&e, f)?,
            Self::Xdr(e) => std::fmt::Display::fmt(&e, f)?,
        };
        Ok(())
    }
}

impl From<Box<dyn error::Error>> for Error {
    fn from(e: Box<dyn error::Error>) -> Self {
        Self::Other(e)
    }
}

impl From<StrValError> for Error {
    fn from(e: StrValError) -> Self {
        Self::StrVal(e)
    }
}

impl From<XdrError> for Error {
    fn from(e: XdrError) -> Self {
        Self::Xdr(e)
    }
}

impl Invoke {
    pub fn run(&self) -> Result<(), Error> {
        let contents = fs::read(&self.file).unwrap();
        let mut h = Host::default();
        let vm = Vm::new(&h, &contents).unwrap();
        let args = self
            .args
            .iter()
            .map(|a| strval::from_string(&h, a))
            .collect::<Result<Vec<ScVal>, StrValError>>()?;
        let res = vm.invoke_function(&mut h, &self.function, &ScVec(args.try_into()?))?;
        let res_str = strval::to_string(&h, res);
        println!("{}", res_str);
        Ok(())
    }
}
