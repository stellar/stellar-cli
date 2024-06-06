pub mod logs;
pub mod start;

// TODO: remove once `network start` is removed
pub type StartCmd = start::Cmd;

#[derive(Debug, clap::Subcommand)]
pub enum Cmd {
    /// Tail logs of a running network container
    Logs(logs::Cmd),
    Start(start::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Logs(#[from] logs::Error),

    #[error(transparent)]
    Start(#[from] start::Error),
}

impl Cmd {
    pub async fn run(&self) -> Result<(), Error> {
        match &self {
            Cmd::Logs(cmd) => cmd.run().await?,
            Cmd::Start(cmd) => cmd.run().await?,
        }
        Ok(())
    }
}
