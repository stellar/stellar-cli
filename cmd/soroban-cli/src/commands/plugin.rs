use std::process::Command;
use which::which;

use crate::utils;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Plugin not provided. Should be `soroban plugin` for a binary `soroban-plugin`")]
    MissingSubcommand,
    #[error(transparent)]
    IO(#[from] std::io::Error),
    #[error(
        r#"no such command: `{0}`
        
        {1}View all installed plugins with `soroban --list`"#
    )]
    ExecutableNotFound(String, String),
    #[error(transparent)]
    Which(#[from] which::Error),
}

pub fn run() -> Result<(), Error> {
    let (name, args) = {
        let mut args = std::env::args().skip(1);
        let name = args.next().ok_or(Error::MissingSubcommand)?;
        (name, args)
    };
    let bin = which(format!("soroban-{name}")).map_err(|_| {
        let suggestion = if let Ok(bins) = list() {
            let suggested_name = bins
                .iter()
                .map(|b| (b, strsim::jaro_winkler(&name, b)))
                .filter(|(_, i)| *i > 0.5f64)
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(a, _)| a.to_string())
                .unwrap_or_default();
            if suggested_name.is_empty() {
                suggested_name
            } else {
                format!(
                    r#"Did you mean `{suggested_name}`?
        "#
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

pub fn list() -> Result<Vec<String>, Error> {
    let re_str = if cfg!(target_os = "windows") {
        r"^soroban-.*.exe$"
    } else {
        r"^soroban-.*"
    };
    let re = regex::Regex::new(re_str).unwrap();
    Ok(which::which_re(re)?
        .filter_map(|b| {
            let s = b.file_name()?.to_str()?;
            Some(s.strip_suffix(".exe").unwrap_or(s).to_string())
        })
        .filter(|s| !(utils::is_hex_string(s) && s.len() > MAX_HEX_LENGTH))
        .map(|s| s.replace("soroban-", ""))
        .collect())
}
