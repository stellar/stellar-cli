use crate::commands::global;

pub mod add;
pub mod remove;

#[derive(Debug, clap::Subcommand)]
pub enum Cmd {
    /// Remove contract alias
    Remove(remove::Cmd),

    /// Add contract alias
    Add(add::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Remove(#[from] remove::Error),

    #[error(transparent)]
    Add(#[from] add::Error),
}

impl Cmd {
    pub fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        match &self {
            Cmd::Remove(remove) => remove.run(global_args)?,
            Cmd::Add(add) => add.run(global_args)?,
        }
        Ok(())
    }
}
