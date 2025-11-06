use std::io::{self, Write};
use std::{env, fmt::Display};

use crate::xdr::{Error as XdrError, Transaction};

use crate::{
    config::network::Network, utils::explorer_url_for_transaction, utils::transaction_hash,
};

const TERMS: &[&str] = &["Apple_Terminal", "vscode", "unknown"];

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

    pub fn clear_previous_line(&self) {
        if !self.quiet {
            if cfg!(windows) {
                eprint!("\x1b[2A\r\x1b[2K");
            } else {
                eprint!("\x1b[1A\x1b[2K\r");
            }

            io::stderr().flush().unwrap();
        }
    }

    fn should_add_additional_space(&self) -> bool {
        let term_program = env::var("TERM_PROGRAM").unwrap_or("unknown".to_string());

        if TERMS.contains(&term_program.as_str()) {
            return true;
        }

        false
    }

    // Some terminals like vscode's and macOS' default terminal will not render
    // the subsequent space if the emoji codepoints size is 2; in this case,
    // we need an additional space. We also need an additional space if `TERM_PROGRAM` is not
    // defined (e.g. vhs running in a docker container).
    pub fn compute_emoji<T: Display + Sized>(&self, emoji: T) -> String {
        if self.should_add_additional_space()
            && (emoji.to_string().chars().count() == 2 || format!("{emoji}") == " ")
        {
            return format!("{emoji} ");
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
create_print_functions!(info, infoln, "ℹ️");
create_print_functions!(link, linkln, "🔗");
create_print_functions!(plus, plusln, "➕");
create_print_functions!(save, saveln, "💾");
create_print_functions!(search, searchln, "🔎");
create_print_functions!(warn, warnln, "⚠️");
create_print_functions!(exclaim, exclaimln, "❗️");
create_print_functions!(arrow, arrowln, "➡️");
create_print_functions!(log, logln, "📔");
create_print_functions!(event, eventln, "📅");
create_print_functions!(blank, blankln, "  ");
create_print_functions!(gear, gearln, "⚙️");
create_print_functions!(dir, dirln, "📁");
