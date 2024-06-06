pub mod logs;
pub mod start;
pub mod stop;

// TODO: remove once `network start` is removed
pub type StartCmd = start::Cmd;
// TODO: remove once `network top` is removed
pub type StopCmd = stop::Cmd;

#[derive(Debug, clap::Subcommand)]
pub enum Cmd {
    /// Tail logs of a running network container
    Logs(logs::Cmd),
    Start(start::Cmd),
    Stop(stop::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Logs(#[from] logs::Error),

    #[error(transparent)]
    Start(#[from] start::Error),

    #[error(transparent)]
    Stop(#[from] stop::Error),
}

impl Cmd {
    pub async fn run(&self) -> Result<(), Error> {
        match &self {
            Cmd::Logs(cmd) => cmd.run().await?,
            Cmd::Start(cmd) => cmd.run().await?,
            Cmd::Stop(cmd) => cmd.run().await?,
        }
        Ok(())
    }
}
