use clap::Subcommand;
use stellar_xdr::cli as xdr;

pub mod token;

#[derive(Debug, Subcommand)]
pub enum Cmd {
    /// Wrap, create, and manage token contracts
    Token(token::Root),

    /// Decode xdr
    Xdr(xdr::Root),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Token(#[from] token::Error),
    #[error(transparent)]
    Xdr(#[from] xdr::Error),
}

impl Cmd {
    pub async fn run(&self) -> Result<(), Error> {
        match &self {
            Cmd::Token(token) => token.run().await?,
            Cmd::Xdr(xdr) => xdr.run()?,
        }
        Ok(())
    }
}
