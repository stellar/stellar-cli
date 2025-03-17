use crate::{
    commands::global,
    config::locator::{self},
    print::Print,
};
use clap::Parser;

#[derive(Debug, Parser)]
pub struct Cmd {
    /// Env variable name to get the value of.
    #[arg()]
    pub name: Option<String>,

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
        let mut vars: Vec<EnvVar> = Vec::new();

        if let Some(v) = EnvVar::get("STELLAR_NETWORK") {
            vars.push(v);
        }

        if let Some(v) = EnvVar::get("STELLAR_ACCOUNT") {
            vars.push(v);
        }

        if let Some(name) = &self.name {
            if let Some(v) = vars.iter().find(|v| &v.key == name) {
                println!("{}", v.value);
            }
            return Ok(());
        }

        if vars.is_empty() {
            print.warnln("No defaults or environment variables set".to_string());
            return Ok(());
        }

        let max_len = vars.iter().map(|v| v.str().len()).max().unwrap_or(0);

        vars.sort();

        for v in vars {
            println!("{:max_len$} # {}", v.str(), v.source);
        }

        Ok(())
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct EnvVar {
    key: String,
    value: String,
    source: String,
}

impl EnvVar {
    fn get(key: &str) -> Option<Self> {
        // The _SOURCE env var is set from cmd/soroban-cli/src/cli.rs#set_env_value_from_config
        let source = std::env::var(format!("{key}_SOURCE"))
            .ok()
            .unwrap_or("env".to_string());

        if let Ok(value) = std::env::var(key) {
            return Some(EnvVar {
                key: key.to_string(),
                value,
                source,
            });
        }

        None
    }

    fn str(&self) -> String {
        format!("{}={}", self.key, self.value)
    }
}
