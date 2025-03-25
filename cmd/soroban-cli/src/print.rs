use std::{env, fmt::Display};

use crate::xdr::{Error as XdrError, Transaction};

use crate::{
    config::network::Network, utils::explorer_url_for_transaction, utils::transaction_hash,
};

const TERMS: &[&str] = &["Apple_Terminal", "vscode"];

#[derive(Clone)]
pub struct Print {
    pub quiet: bool,
}

impl Print {
    pub fn new(quiet: bool) -> Print {
        Print { quiet }
    }

    pub fn print<T: Display + Sized>(&self, message: T) {
        if !self.quiet {
            eprint!("{message}");
        }
    }

    pub fn println<T: Display + Sized>(&self, message: T) {
        if !self.quiet {
            eprintln!("{message}");
        }
    }

    pub fn clear_line(&self) {
        if cfg!(windows) {
            eprint!("\r");
        } else {
            eprint!("\r\x1b[2K");
        }
    }

    // Some terminals like vscode's and macOS' default terminal will not render
    // the subsequent space if the emoji codepoints size is 2; in this case,
    // we need an additional space.
    pub fn compute_emoji<T: Display + Sized>(&self, emoji: T) -> String {
        if let Ok(term_program) = env::var("TERM_PROGRAM") {
            if TERMS.contains(&term_program.as_str())
                && (emoji.to_string().chars().count() == 2 || format!("{emoji}") == " ")
            {
                return format!("{emoji} ");
            }
        }

        emoji.to_string()
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

        self.infoln(format!("Transaction hash is {hash}").as_str());

        if show_link {
            if let Some(url) = explorer_url_for_transaction(network, &hash) {
                self.linkln(url);
            }
        }

        Ok(())
    }
}

macro_rules! create_print_functions {
    ($name:ident, $nameln:ident, $icon:expr) => {
        impl Print {
            #[allow(dead_code)]
            pub fn $name<T: Display + Sized>(&self, message: T) {
                if !self.quiet {
                    eprint!("{} {}", self.compute_emoji($icon), message);
                }
            }

            #[allow(dead_code)]
            pub fn $nameln<T: Display + Sized>(&self, message: T) {
                if !self.quiet {
                    eprintln!("{} {}", self.compute_emoji($icon), message);
                }
            }
        }
    };
}

create_print_functions!(bucket, bucketln, "🪣");
create_print_functions!(check, checkln, "✅");
create_print_functions!(error, errorln, "❌");
create_print_functions!(globe, globeln, "🌎");
create_print_functions!(info, infoln, "ℹ️"); // ℹ
create_print_functions!(link, linkln, "🔗");
create_print_functions!(plus, plusln, "➕");
create_print_functions!(save, saveln, "💾");
create_print_functions!(search, searchln, "🔎");
create_print_functions!(warn, warnln, "⚠️"); // ⚠
create_print_functions!(exclaim, exclaimln, "❗️"); // ❗
create_print_functions!(arrow, arrowln, "➡️"); // ➡
create_print_functions!(log, logln, "📔");
create_print_functions!(event, eventln, "📅");
create_print_functions!(blank, blankln, " ");
