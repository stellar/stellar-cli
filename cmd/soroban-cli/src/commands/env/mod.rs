use crate::{
    commands::global,
    config::locator::{self, config},
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

    #[error("no defaults or environment variables set")]
    NoEnv,
}

impl Cmd {
    pub fn run(&self, _global_args: &global::Args) -> Result<(), Error> {
        let config = config()?;
        let mut lines: Vec<(String, String)> = Vec::new();

        if let Some(data) = get("STELLAR_NETWORK", config.defaults.network) {
            lines.push(data);
        }

        if let Some(data) = get("STELLAR_ACCOUNT", config.defaults.identity) {
            lines.push(data);
        }

        if lines.is_empty() {
            return Err(Error::NoEnv);
        }

        let max_len = lines.iter().map(|l| l.0.len()).max().unwrap_or(0);

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
