use super::global;

mod max;
mod min;

#[derive(Debug, clap::Subcommand)]
pub enum Cmd {
    #[command()]
    Max(max::Cmd),
    #[command()]
    Min(min::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Max(#[from] max::Error),
    #[error(transparent)]
    Min(#[from] min::Error),
}

impl Cmd {
    pub fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        match self {
            Cmd::Min(cmd) => cmd.run(global_args)?,
            Cmd::Max(cmd) => cmd.run(global_args)?,
        };
        Ok(())
    }
}
