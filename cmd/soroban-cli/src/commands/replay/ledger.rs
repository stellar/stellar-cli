use super::{args, Error};
use crate::commands::global;

/// Replay a ledger with stellar-core and output its meta
///
/// Outputs the ledger's `LedgerCloseMeta`, the record of what closing the
/// ledger did: the transactions applied, their results, the ledger entries they
/// changed, and the events they emitted, including diagnostic events that show
/// the contract calls made and the errors raised.
///
/// The ledger is replayed by stellar-core, which must be installed, from the
/// network's history archive, starting from the ledger state at the checkpoint
/// before the ledger. For mainnet the state is several GB to download and needs
/// tens of GB of disk. The state is kept in the cache directory, and a ledger
/// shortly after the last one replayed continues from it. Ledgers replayed are
/// also kept, and aren't replayed again.
#[derive(Debug, clap::Parser)]
pub struct Cmd {
    /// Ledger sequence number to replay
    #[arg(long)]
    pub ledger: u32,

    #[command(flatten)]
    pub args: args::Args,

    /// Format of the output
    #[arg(long, value_enum, default_value_t)]
    pub output: args::OutputFormat,
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let meta = self.args.replay(self.ledger, global_args).await?;
        self.output.print(&meta)
    }
}
