use itertools::Itertools;
use std::{path::PathBuf, process::Command};
use which::which;

use crate::utils;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    IO(#[from] std::io::Error),

    #[error(transparent)]
    Which(#[from] which::Error),

    #[error(transparent)]
    Regex(#[from] regex::Error),
}

pub fn run() -> Result<(), Error> {
    if let Some((plugin_bin, args)) = find_plugin() {
        std::process::exit(
            Command::new(plugin_bin)
                .args(args)
                .spawn()?
                .wait()?
                .code()
                .unwrap(),
        );
    }

    Ok(())
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
        .unique()
        .collect())
}

fn find_plugin() -> Option<(PathBuf, Vec<String>)> {
    let args_vec: Vec<String> = std::env::args().skip(1).collect();
    let mut chain: Vec<String> = args_vec
        .iter()
        .take_while(|arg| !arg.starts_with("--"))
        .map(ToString::to_string)
        .collect();

    while !chain.is_empty() {
        let name = chain.join("-");
        let bin = find_bin(&name).ok();

        if let Some(bin) = &bin {
            let index = chain.len();
            let args = args_vec[index..]
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<String>>();

            return Some((bin.into(), args));
        }

        chain.pop();
    }

    None
}
