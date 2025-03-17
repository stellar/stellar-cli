use super::global;

mod next;

#[derive(Debug, clap::Subcommand)]
pub enum Cmd {

    /// Fetch the source account's seq-num and increment for the given tx
    #[command()]
    Next(next::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Next(#[from] next::Error),
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        match self {
            Cmd::Next(cmd) => cmd.run(global_args).await?,
        };
        Ok(())
    }
}
