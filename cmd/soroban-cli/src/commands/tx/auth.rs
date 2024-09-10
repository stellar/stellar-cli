use clap::arg;

use crate::rpc::{self, Client};

#[derive(Debug, clap::Args, Clone)]
#[group(skip)]
pub struct Args {
    /// Number of ledgers from current ledger before the signed auth entry expires. Default 60 ~ 5 minutes.
    #[arg(long, default_value = "60")]
    pub auth_expires_in_ledgers: u32,
    /// Ledger number when signed auth entry expires.
    #[arg(long, conflicts_with = "auth_expires_in_ledgers")]
    pub auth_expires_at_ledger: Option<u32>,
}

impl Args {
    pub async fn expiration_ledger(&self, client: &Client) -> Result<u32, rpc::Error> {
        if let Some(ledger) = self.auth_expires_at_ledger {
            return Ok(ledger);
        }
        let current_ledger = client.get_latest_ledger().await?.sequence;
        Ok(current_ledger + self.auth_expires_in_ledgers)
    }
}

impl Default for Args {
    fn default() -> Self {
        Self {
            auth_expires_in_ledgers: 60,
            auth_expires_at_ledger: None,
        }
    }
}
