use clap::arg;

#[derive(Debug, clap::Args, Clone)]
#[group(skip)]
pub struct Args {
    /// Number of ledgers from current ledger before the signed auth entry expires. Default 60 ~ 5 minutes.
    #[arg(
        long = "ledgers-from-now",
        visible_alias = "ledgers",
        default_value = "60"
    )]
    pub from_now: u32,
}

impl Default for Args {
    fn default() -> Self {
        Self { from_now: 60 }
    }
}
