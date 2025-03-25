use clap::CommandFactory;
use dotenvy::dotenv;
use tracing_subscriber::{fmt, EnvFilter};
use crate::commands::{self, contract::arg_parsing::Error::HelpMessage};
use clap::Parser;
use std::error::Error as StdError;
use std::process;
use tokio;

use crate::commands::contract::deploy::wasm;
use crate::commands::contract::deploy::Error as DeployError;
use crate::commands::contract::invoke;
use crate::commands::contract::Error as ContractError;
use crate::commands::Error as CommandsError;
use crate::config::Config;
use crate::print::Print;
use crate::upgrade_check::upgrade_check;
use crate::Root;

#[derive(Debug, thiserror::Error)]
pub enum CliError {
    #[error(transparent)]
    Commands(#[from] commands::Error),
}

#[derive(Parser, Debug)]
#[command(
    name = "stellar",
    about = commands::ABOUT,
    version = commands::version::long(),
    long_about = commands::ABOUT.to_string() + commands::LONG_ABOUT,
    disable_help_subcommand = true,
)]
pub struct Cli {
    #[clap(flatten)]
    pub global_args: commands::global::Args,

    #[command(subcommand)]
    pub cmd: commands::Cmd,
}

impl Cli {
    pub fn new() -> Result<Self, commands::Error> {
        Self::try_parse().map_err(|e| {
            if std::env::args().any(|s| s == "--list") {
                let plugins = commands::plugin::list().unwrap_or_default();
                if plugins.is_empty() {
                    println!("No Plugins installed. E.g. soroban-hello");
                } else {
                    println!("Installed Plugins:\n    {}", plugins.join("\n    "));
                }
                std::process::exit(0);
            }
            match e.kind() {
                clap::error::ErrorKind::InvalidSubcommand => match commands::plugin::run() {
                    Ok(()) => commands::Error::Clap(e),
                    Err(e) => commands::Error::Plugin(e),
                },
                _ => commands::Error::Clap(e),
            }
        })
    }

    pub async fn run(&self) -> Result<(), commands::Error> {
        match &self.cmd {
            commands::Cmd::Completion(completion) => completion.run().map_err(commands::Error::from),
            commands::Cmd::Contract(contract) => contract.run(&self.global_args).await.map_err(commands::Error::Contract),
            commands::Cmd::Policy(policy) => policy.run().await.map_err(commands::Error::from),
        }
    }
}

pub async fn cli_main() -> Result<(), commands::Error> {
    let root = commands::Root::parse();
    match root.cmd {
        commands::Cmd::Policy(policy) => policy.run().await.map_err(commands::Error::from),
        commands::Cmd::Contract(contract) => contract.run(&root.global_args).await.map_err(commands::Error::Contract),
        commands::Cmd::Completion(completion) => completion.run().map_err(commands::Error::from),
    }
}

#[tokio::main]
pub async fn main() {
    let root = commands::Root::parse();
    match root.cmd {
        commands::Cmd::Policy(policy) => {
            if let Err(e) = policy.run().await {
                eprintln!("Error: {}", e);
                process::exit(1);
            }
        },
        commands::Cmd::Contract(contract) => {
            if let Err(e) = contract.run(&root.global_args).await {
                eprintln!("Error: {}", e);
                process::exit(1);
            }
        },
        commands::Cmd::Completion(completion) => {
            if let Err(e) = completion.run() {
                eprintln!("Error: {}", e);
                process::exit(1);
            }
        },
    }
}

// Load ~/.config/stellar/config.toml defaults as env vars.
fn set_env_from_config() {
    if let Ok(config) = Config::new() {
        set_env_value_from_config("STELLAR_ACCOUNT", config.defaults.identity);
        set_env_value_from_config("STELLAR_NETWORK", config.defaults.network);
    }
}

// Set an env var from a config file if the env var is not already set.
// Additionally, a `$NAME_SOURCE` variant will be set, which allows
// `stellar env` to properly identity the source.
fn set_env_value_from_config(name: &str, value: Option<String>) {
    let Some(value) = value else {
        return;
    };

    std::env::remove_var(format!("{name}_SOURCE"));

    if std::env::var(name).is_err() {
        std::env::set_var(name, value);
        std::env::set_var(format!("{name}_SOURCE"), "use");
    }
}
