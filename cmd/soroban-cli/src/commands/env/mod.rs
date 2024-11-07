use crate::{
    commands::global,
    config::{
        locator::{self},
        Config,
    },
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
        let config = Config::new()?;
        let mut lines: Vec<(String, String)> = Vec::new();

        if let Some(data) = get("STELLAR_NETWORK", config.defaults.network) {
            lines.push(data);
        }

        if let Some(data) = get("STELLAR_ACCOUNT", config.defaults.identity) {
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

fn get(env_var: &str, default_value: Option<String>) -> Option<(String, String)> {
    if let Ok(value) = std::env::var(env_var) {
        return Some((format!("{env_var}={value}"), "env".to_string()));
    }

    if let Some(value) = default_value {
        return Some((format!("{env_var}={value}"), "default".to_string()));
    }

    None
}
