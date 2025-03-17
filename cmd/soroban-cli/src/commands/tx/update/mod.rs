use super::global;

pub mod sequence_number;

#[derive(Debug, clap::Subcommand)]
pub enum Cmd {
    /// Edit the sequence number on a transaction
    #[command(subcommand, visible_alias = "seq-num")]
    SequenceNumber(sequence_number::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    SequenceNumber(#[from] sequence_number::Error),
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        match self {
            Cmd::SequenceNumber(cmd) => cmd.run(global_args).await?,
        };
        Ok(())
    }
}
