use std::{path::PathBuf, process::Command};

use clap::CommandFactory;
use which::which;

use crate::{utils, Root};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Plugin not provided. Should be `stellar plugin` for a binary `stellar-plugin`")]
    MissingSubcommand,
    #[error(transparent)]
    IO(#[from] std::io::Error),
    #[error(
        r"error: no such command: `{0}`
        
        {1}View all installed plugins with `stellar --list`"
    )]
    ExecutableNotFound(String, String),
    #[error(transparent)]
    Which(#[from] which::Error),
    #[error(transparent)]
    Regex(#[from] regex::Error),
}

const SUBCOMMAND_TOLERANCE: f64 = 0.75;
const PLUGIN_TOLERANCE: f64 = 0.75;
const MIN_LENGTH: usize = 4;

/// Tries to run a plugin, if the plugin's name is similar enough to any of the current subcommands return Ok.
/// Otherwise only errors can be returned because this process will exit with the plugin.
pub fn run() -> Result<(), Error> {
    let (name, args) = {
        let mut args = std::env::args().skip(1);
        let name = args.next().ok_or(Error::MissingSubcommand)?;
        (name, args)
    };

    if Root::command().get_subcommands().any(|c| {
        let sc_name = c.get_name();
        sc_name.starts_with(&name)
            || (name.len() >= MIN_LENGTH && strsim::jaro(sc_name, &name) >= SUBCOMMAND_TOLERANCE)
    }) {
        return Ok(());
    }

    let bin = find_bin(&name).map_err(|_| {
        let suggestion = if let Ok(bins) = list() {
            let suggested_name = bins
                .iter()
                .map(|b| (b, strsim::jaro_winkler(&name, b)))
                .filter(|(_, i)| *i > PLUGIN_TOLERANCE)
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(a, _)| a.to_string())
                .unwrap_or_default();

            if suggested_name.is_empty() {
                suggested_name
            } else {
                format!(
                    r"Did you mean `{suggested_name}`?
        "
                )
            }
        } else {
            String::new()
        };

        Error::ExecutableNotFound(name, suggestion)
    })?;

    std::process::exit(
        Command::new(bin)
            .args(args)
            .spawn()?
            .wait()?
            .code()
            .unwrap(),
    );
}

const MAX_HEX_LENGTH: usize = 10;

fn find_bin(name: &str) -> Result<PathBuf, which::Error> {
    if let Ok(path) = which(format!("stellar-{name}")) {
        Ok(path)
    } else {
        which(format!("soroban-{name}"))
    }
}

pub fn list() -> Result<Vec<String>, Error> {
    let re_str = if cfg!(target_os = "windows") {
        r"^(soroban|stellar)-.*.exe$"
    } else {
        r"^(soroban|stellar)-.*"
    };

    let re = regex::Regex::new(re_str)?;

    Ok(which::which_re(re)?
        .filter_map(|b| {
            let s = b.file_name()?.to_str()?;
            Some(s.strip_suffix(".exe").unwrap_or(s).to_string())
        })
        .filter(|s| !(utils::is_hex_string(s) && s.len() > MAX_HEX_LENGTH))
        .map(|s| s.replace("soroban-", "").replace("stellar-", ""))
        .collect())
}
