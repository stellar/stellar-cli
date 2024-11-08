use crate::{
    commands::global,
    config::locator::{self},
    print::Print,
};
use clap::Parser;

#[derive(Debug, Parser)]
pub struct Cmd {
    #[command(flatten)]
    pub config_locator: locator::Args,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Locator(#[from] locator::Error),
}

impl Cmd {
    pub fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let print = Print::new(global_args.quiet);
        let mut lines: Vec<(String, String)> = Vec::new();

        if let Some(data) = get("STELLAR_NETWORK") {
            lines.push(data);
        }

        if let Some(data) = get("STELLAR_ACCOUNT") {
            lines.push(data);
        }

        if lines.is_empty() {
            print.warnln("No defaults or environment variables set".to_string());
            return Ok(());
        }

        let max_len = lines.iter().map(|l| l.0.len()).max().unwrap_or(0);

        lines.sort();

        for (value, source) in lines {
            println!("{value:max_len$} # {source}");
        }

        Ok(())
    }
}

fn get(env_var: &str) -> Option<(String, String)> {
    // The _SOURCE env var is set from cmd/soroban-cli/src/cli.rs#set_env_value_from_config
    let source = std::env::var(format!("{env_var}_SOURCE"))
        .ok()
        .unwrap_or("env".to_string());

    if let Ok(value) = std::env::var(env_var) {
        return Some((format!("{env_var}={value}"), source));
    }

    None
}
