use std::io::{self, Write};
use std::sync::OnceLock;
use std::{env, fmt::Display};

use crate::xdr::{Error as XdrError, Transaction};

use crate::{
    config::network::Network, utils::explorer_url_for_transaction, utils::transaction_hash,
};

static QUIET: OnceLock<bool> = OnceLock::new();

/// Record whether `--quiet` was passed on the command line. Called once from
/// `cli::main` after parsing so resolvers running deep in the call stack can
/// honor the global flag without it being threaded through every signature.
pub fn set_quiet(quiet: bool) {
    let _ = QUIET.set(quiet);
}

/// Read the recorded `--quiet` flag, defaulting to `false` if [`set_quiet`]
/// hasn't run yet (e.g. in unit tests that don't go through `cli::main`).
pub fn is_quiet() -> bool {
    *QUIET.get().unwrap_or(&false)
}

#[derive(Clone)]
pub struct Print {
    pub quiet: bool,
}

impl Print {
    pub fn new(quiet: bool) -> Print {
        Print { quiet }
    }

    /// Print message to stderr if not in quiet mode
    pub fn print<T: Display + Sized>(&self, message: T) {
        if !self.quiet {
            eprint!("{message}");
        }
    }

    /// Print message with newline to stderr if not in quiet mode.
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

    // Some terminals like vscode's and macOS' default terminal will not render
    // the subsequent space if the emoji codepoints size is 2; in this case,
    // we need an additional space. We also need an additional space if `TERM_PROGRAM` is not
    // defined (e.g. vhs running in a docker container).
    pub fn compute_emoji<T: Display + Sized>(&self, emoji: T) -> String {
        if should_add_additional_space()
            && (emoji.to_string().chars().count() == 2 || format!("{emoji}") == " ")
        {
            return format!("{emoji} ");
        }

        emoji.to_string()
    }

    pub fn log_explorer_url(&self, network: &Network, tx_hash: &str) {
        if let Some(url) = explorer_url_for_transaction(network, tx_hash) {
            self.linkln(url);
        }
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
            self.log_explorer_url(network, &hash);
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

/// Format a number with the appropriate number of decimals, trimming trailing zeros.
///
/// If `n` cannot be represented as an i128 value, returns "Err(number out of bounds)".
pub fn format_number<T: TryInto<i128>>(n: T, decimals: u32) -> String {
    let n: i128 = match n.try_into() {
        Ok(value) => value,
        Err(_) => return "Err(number out of bounds)".to_string(),
    };
    if decimals == 0 {
        return n.to_string();
    }
    let divisor = 10i128.pow(decimals);
    let integer_part = n / divisor;
    let fractional_part = (n % divisor).abs();
    // Pad with leading zeros to match decimals width, then trim trailing zeros
    let frac_str = format!("{:0width$}", fractional_part, width = decimals as usize);
    let frac_trimmed = frac_str.trim_end_matches('0');

    if frac_trimmed.is_empty() {
        format!("{integer_part}")
    } else {
        // If integer_part is 0, we still want to show the sign for negative numbers (e.g. -0.5)
        let sign = if n < 0 && integer_part == 0 { "-" } else { "" };
        format!("{sign}{integer_part}.{frac_trimmed}")
    }
}

fn should_add_additional_space() -> bool {
    const TERMS: &[&str] = &["Apple_Terminal", "vscode", "unknown"];
    let term_program = env::var("TERM_PROGRAM").unwrap_or("unknown".to_string());

    if TERMS.contains(&term_program.as_str()) {
        return true;
    }

    false
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::unreadable_literal)]
    fn test_format_number() {
        assert_eq!(format_number(0i128, 7), "0");
        assert_eq!(format_number(1234567i128, 7), "0.1234567");
        assert_eq!(format_number(12345000i128, 7), "1.2345");
        assert_eq!(format_number(10000000i128, 7), "1");
        assert_eq!(format_number(123456789012345i128, 7), "12345678.9012345");
        assert_eq!(format_number(-1234567i128, 7), "-0.1234567");
        assert_eq!(format_number(-12345000i128, 7), "-1.2345");
        assert_eq!(format_number(12345i128, 0), "12345");
        assert_eq!(format_number(12345i128, 1), "1234.5");
        assert_eq!(format_number(1i128, 7), "0.0000001");

        assert_eq!(format_number(1u32, 7), "0.0000001");
        assert_eq!(format_number(1i32, 7), "0.0000001");
        assert_eq!(format_number(1u64, 7), "0.0000001");
        assert_eq!(format_number(1i64, 7), "0.0000001");
        assert_eq!(format_number(1u128, 7), "0.0000001");

        let err: u128 = u128::try_from(i128::MAX).unwrap() + 1;
        let result = format_number(err, 0);
        assert_eq!(result, "Err(number out of bounds)");

        let min: i128 = i128::MIN;
        let result = format_number(min, 18);
        assert_eq!(result, "-170141183460469231731.687303715884105728");

        let max: i128 = i128::MAX;
        let result = format_number(max, 18);
        assert_eq!(result, "170141183460469231731.687303715884105727");
    }
}
