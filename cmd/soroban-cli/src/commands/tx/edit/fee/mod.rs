use super::global;

mod set;

#[derive(Debug, clap::Subcommand)]
pub enum Cmd {
    /// Set the transaction fee
    #[command()]
    Set(set::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Set(#[from] set::Error),
}

impl Cmd {
    pub fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        match self {
            Cmd::Set(cmd) => cmd.run(global_args)?,
        };
        Ok(())
    }
}
