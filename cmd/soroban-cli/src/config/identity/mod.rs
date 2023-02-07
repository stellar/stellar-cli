use clap::Parser;

pub mod add;
pub mod generate;
pub mod ls;
pub mod rm;

#[derive(Debug, Parser)]
pub enum Cmd {
    /// Add a new identity (keypair, ledger, macOS keychain)
    Add(add::Cmd),
    /// Generate a new identity with a seed phrase, currently 12 words
    Generate(generate::Cmd),
    /// List identities
    Ls(ls::Cmd),
    /// Remove an identity
    Rm(rm::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Add(#[from] add::Error),

    #[error(transparent)]
    Generate(#[from] generate::Error),

    #[error(transparent)]
    Rm(#[from] rm::Error),

    #[error(transparent)]
    Ls(#[from] ls::Error),
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        match self {
            Cmd::Add(cmd) => cmd.run()?,
            Cmd::Rm(new) => new.run()?,
            Cmd::Ls(cmd) => cmd.run()?,
            Cmd::Generate(cmd) => cmd.run()?,
        };
        Ok(())
    }
}
