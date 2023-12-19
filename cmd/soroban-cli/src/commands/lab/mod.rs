use clap::Subcommand;

pub mod token;

#[derive(Debug, Subcommand)]
pub enum Cmd {
    /// Wrap, create, and manage token contracts
    Token(token::Root),

    /// Decode xdr
    Xdr(stellar_xdr::cli::Root),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Token(#[from] token::Error),
    #[error(transparent)]
    Xdr(#[from] stellar_xdr::cli::Error),
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
