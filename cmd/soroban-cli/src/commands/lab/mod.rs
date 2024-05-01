use clap::Subcommand;
use stellar_xdr::cli as xdr;

#[derive(Debug, Subcommand)]
pub enum Cmd {
    /// Decode xdr
    Xdr(xdr::Root),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Xdr(#[from] xdr::Error),
}

impl Cmd {
    pub async fn run(&self) -> Result<(), Error> {
        match &self {
            Cmd::Xdr(xdr) => xdr.run()?,
        }
        Ok(())
    }
}
