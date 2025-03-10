use super::global;

mod increment;

#[derive(Debug, clap::Subcommand)]
pub enum Cmd {
    /// Increase the transaction's sequence number
    #[command(visible_alias = "inc")]
    Increment(increment::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Increment(#[from] increment::Error),
}

impl Cmd {
    pub fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        match self {
            Cmd::Increment(cmd) => cmd.run(global_args)?,
        };
        Ok(())
    }
}
