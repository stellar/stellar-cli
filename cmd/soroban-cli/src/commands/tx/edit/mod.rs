use super::global;

pub mod fee;
pub mod memo;
pub mod source_account;
pub mod sequence_number;
pub mod time_bound;
mod precondition;

#[derive(Debug, clap::Subcommand)]
pub enum Cmd {
    /// Set the fee on a transaction
    #[command(subcommand)]
    Fee(fee::Cmd),
    /// Set the memo on a transaction
    #[command(subcommand)]
    Memo(memo::Cmd),
    /// Change the source account on a transaction
    #[command(subcommand, visible_alias = "source")]
    SourceAccount(source_account::Cmd),
    /// Set the sequence number on a transaction
    #[command(subcommand, visible_alias = "seq-num")]
    SequenceNumber(sequence_number::Cmd),
    #[command(subcommand)]
    TimeBound(time_bound::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Fee(#[from] fee::Error),
    #[error(transparent)]
    Memo(#[from] memo::Error),
    #[error(transparent)]
    SourceAccount(#[from] source_account::Error),
    #[error(transparent)]
    SequenceNumber(#[from] sequence_number::Error),
    #[error(transparent)]
    TimeBound(#[from] time_bound::Error)
}

impl Cmd {
    pub fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        match self {
            Cmd::Fee(cmd) => cmd.run(global_args)?,
            Cmd::Memo(cmd) => cmd.run(global_args)?,
            Cmd::SourceAccount(cmd) => cmd.run(global_args)?,
            Cmd::SequenceNumber(cmd) => cmd.run(global_args)?,
        };
        Ok(())
    }
}