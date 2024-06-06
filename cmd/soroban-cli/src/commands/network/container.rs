pub mod logs;

#[derive(Debug, clap::Subcommand)]
pub enum Cmd {
    /// Tail logs of a running network container
    Logs(logs::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Logs(#[from] logs::Error),
}

impl Cmd {
    pub async fn run(&self) -> Result<(), Error> {
        match &self {
            Cmd::Logs(cmd) => cmd.run().await?,
        }
        Ok(())
    }
}
