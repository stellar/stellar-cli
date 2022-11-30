use clap::Parser;

pub mod add;
pub mod default;
pub mod ls;
pub mod rm;

#[derive(Debug, Parser)]
pub enum Cmd {
    /// Add a new identity (keypair, ledger, macOS keychain)
    Add(add::Cmd),
    /// Remove an identity
    Rm(rm::Cmd),
    /// List identities
    Ls(ls::Cmd),
    /// Set a default identity
    Default(default::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Add(#[from] add::Error),

    #[error(transparent)]
    Rm(#[from] rm::Error),

    #[error(transparent)]
    Ls(#[from] ls::Error),

    #[error(transparent)]
    Default(#[from] default::Error),
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        match &self {
            Cmd::Add(cmd) => cmd.run()?,
            Cmd::Rm(new) => new.run()?,
            Cmd::Ls(cmd) => cmd.run()?,
            Cmd::Default(use_cmd) => use_cmd.run()?,
        };
        Ok(())
    }
}
