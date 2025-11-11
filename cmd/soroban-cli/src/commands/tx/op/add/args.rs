use crate::config::address;
use std::ffi::OsString;

#[derive(Debug, clap::Args, Clone)]
#[group(skip)]
pub struct Args {
    /// Source account used for the operation
    #[arg(
        long,
        visible_alias = "op-source",
        env = "STELLAR_OPERATION_SOURCE_ACCOUNT"
    )]
    pub operation_source_account: Option<address::UnresolvedMuxedAccount>,
    /// Base-64 transaction envelope XDR or file containing XDR to decode, or stdin if empty
    #[arg()]
    pub tx_xdr: Option<OsString>,
}

impl Args {
    pub fn source(&self) -> Option<&address::UnresolvedMuxedAccount> {
        self.operation_source_account.as_ref()
    }
}
