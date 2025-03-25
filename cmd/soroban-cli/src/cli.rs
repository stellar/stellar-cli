use clap::CommandFactory;
use dotenvy::dotenv;
use tracing_subscriber::{fmt, EnvFilter};
use crate::commands::{self, contract::arg_parsing::Error::HelpMessage};
use clap::Parser;
use std::error::Error as StdError;

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
            commands::Cmd::Policy(policy) => policy.run().map_err(commands::Error::from),
        }
    }
}

#[tokio::main]
pub async fn main() {
    let _ = dotenv().unwrap_or_default();

    // Map SOROBAN_ env vars to STELLAR_ env vars for backwards compatibility
    // with the soroban-cli prior to when the stellar-cli was released.
    let vars = &[
        "FEE",
        "NO_CACHE",
        "ACCOUNT",
        "CONTRACT_ID",
        "INVOKE_VIEW",
        "RPC_URL",
        "NETWORK_PASSPHRASE",
        "NETWORK",
        "PORT",
        "SECRET_KEY",
        "CONFIG_HOME",
    ];
    for var in vars {
        let soroban_key = format!("SOROBAN_{var}");
        let stellar_key = format!("STELLAR_{var}");
        if let Ok(val) = std::env::var(soroban_key) {
            std::env::set_var(stellar_key, val);
        }
    }

    set_env_from_config();

    let mut root = Root::new().unwrap_or_else(|e| match e {
        commands::Error::Clap(e) => {
            let mut cmd = Root::command();
            e.format(&mut cmd).exit();
        }
        e => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    });

    // Now use root to setup the logger
    if let Some(level) = root.global_args.log_level() {
        let mut e_filter = EnvFilter::from_default_env()
            .add_directive("hyper=off".parse().unwrap())
            .add_directive(format!("stellar_cli={level}").parse().unwrap())
            .add_directive("stellar_rpc_client=off".parse().unwrap())
            .add_directive(format!("soroban_cli={level}").parse().unwrap());

        for filter in &root.global_args.filter_logs {
            e_filter = e_filter.add_directive(
                filter
                    .parse()
                    .map_err(|e| {
                        eprintln!("{e}: {filter}");
                        std::process::exit(1);
                    })
                    .unwrap(),
            );
        }

        let builder = fmt::Subscriber::builder()
            .with_env_filter(e_filter)
            .with_ansi(false)
            .with_writer(std::io::stderr);

        let subscriber = builder.finish();
        tracing::subscriber::set_global_default(subscriber)
            .expect("Failed to set the global tracing subscriber");
    }

    // Spawn a thread to check if a new version exists.
    // It depends on logger, so we need to place it after
    // the code block that initializes the logger.
    tokio::spawn(async move {
        upgrade_check(root.global_args.quiet).await;
    });

    let printer = Print::new(root.global_args.quiet);
    if let Err(e) = root.run().await {
        match e {
            CommandsError::Contract(ContractError::Invoke(invoke::Error::ArgParsing(HelpMessage(help)))) => {
                println!("{help}");
                std::process::exit(1);
            }
            CommandsError::Contract(ContractError::Deploy(DeployError::Wasm(wasm::Error::ArgParse(HelpMessage(help))))) => {
                println!("{help}");
                std::process::exit(1);
            }
            _ => {
                printer.errorln(format!("error: {e}"));
                std::process::exit(1);
            }
        }
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
