use clap::Subcommand;

pub mod token;
pub mod xdr;

#[derive(Debug, Subcommand)]
pub enum SubCmd {
    /// Wrap, create, and manage token contracts
    Token(token::Root),

    /// Decode xdr
    Xdr(xdr::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Token(#[from] token::Error),
    #[error(transparent)]
    Xdr(#[from] xdr::Error),
}

impl SubCmd {
    pub async fn run(&self) -> Result<(), Error> {
        match &self {
            SubCmd::Token(token) => token.run().await?,
            SubCmd::Xdr(xdr) => xdr.run()?,
        }
        Ok(())
    }
}
