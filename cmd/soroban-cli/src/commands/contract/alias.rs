use crate::commands::global;

pub mod remove;

#[derive(Debug, clap::Subcommand)]
pub enum Cmd {
    /// Remove contract alias
    Remove(remove::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Remove(#[from] remove::Error),
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        match &self {
            Cmd::Remove(remove) => remove.run(global_args).await?,
        }
        Ok(())
    }
}
