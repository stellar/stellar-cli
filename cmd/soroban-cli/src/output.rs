use std::fmt::Display;

use soroban_env_host::xdr::{Error as XdrError, Transaction};

use crate::{
    config::network::Network,
    utils::{explorer_url_for_transaction, transaction_hash},
};

pub struct Output {
    pub quiet: bool,
}

impl Output {
    pub fn new(quiet: bool) -> Output {
        Output { quiet }
    }

    pub fn print<T: Display>(&self, icon: &str, message: T, new_line: bool) {
        if self.quiet {
            return;
        }

        if new_line {
            eprintln!("{icon} {message}");
        } else {
            eprint!("{icon} {message}");
        }
    }

    pub fn check<T: Display>(&self, message: T) {
        self.print("✅", message, true);
    }

    pub fn search<T: Display>(&self, message: T) {
        self.print("🔎", message, true);
    }

    pub fn save<T: Display>(&self, message: T) {
        self.print("💾", message, true);
    }

    pub fn bucket<T: Display>(&self, message: T) {
        self.print("🪣", message, true);
    }

    pub fn info<T: Display>(&self, message: T) {
        self.print("ℹ️", message, true);
    }

    pub fn globe<T: Display>(&self, message: T) {
        self.print("🌎", message, true);
    }

    pub fn link<T: Display>(&self, message: T) {
        self.print("🔗", message, true);
    }

    /// # Errors
    ///
    /// Might return an error
    pub fn log_transaction(
        &self,
        tx: &Transaction,
        network: &Network,
        show_link: bool,
    ) -> Result<(), XdrError> {
        let tx_hash = transaction_hash(tx, &network.network_passphrase)?;
        let hash = hex::encode(tx_hash);

        self.info(format!("Transaction hash is {hash}").as_str());

        if show_link {
            if let Some(url) = explorer_url_for_transaction(network, &hash) {
                self.link(url);
            }
        }

        Ok(())
    }
}
