use clap::CommandFactory;
use dotenvy::dotenv;
use std::path::PathBuf;
use tracing_subscriber::{fmt, EnvFilter};

use crate::commands::contract::arg_parsing::Error::HelpMessage;
use crate::commands::contract::deploy::wasm::Error::ArgParse;
use crate::commands::contract::invoke::Error::ArgParsing;
use crate::commands::contract::Error::{Deploy, Invoke};
use crate::commands::Error::Contract;
use crate::config::{locator::cli_config_file, Config};
use crate::print::Print;
use crate::upgrade_check::upgrade_check;
use crate::{commands, env_vars, Root};
use std::error::Error;

#[tokio::main]
pub async fn main() {
    let _ = dotenv().unwrap_or_default();

    // Map SOROBAN_ env vars to STELLAR_ env vars for backwards compatibility
    // with the soroban-cli prior to when the stellar-cli was released.
    //
    let vars = env_vars::unprefixed();

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
        // TODO: source is None (should be HelpMessage)
        let _source = commands::Error::source(&e);
        // TODO use source instead
        if let Contract(Invoke(ArgParsing(HelpMessage(help)))) = e {
            println!("{help}");
            std::process::exit(1);
        }
        if let Contract(Deploy(ArgParse(HelpMessage(help)))) = e {
            println!("{help}");
            std::process::exit(1);
        }
        printer.errorln(format!("error: {e}"));
        std::process::exit(1);
    }
}

// Load config.toml defaults as env vars, honoring --config-dir if present in raw args.
fn set_env_from_config() {
    let config_file = config_dir_from_raw_args()
        .map(|dir| dir.join("config.toml"))
        .or_else(|| cli_config_file().ok());

    let config = config_file
        .as_deref()
        .and_then(|p| Config::load(p).ok())
        .unwrap_or_default();

    set_env_value_from_config("STELLAR_ACCOUNT", config.defaults.identity);
    set_env_value_from_config("STELLAR_NETWORK", config.defaults.network);
    set_env_value_from_config("STELLAR_INCLUSION_FEE", config.defaults.inclusion_fee);
}

fn config_dir_from_raw_args() -> Option<PathBuf> {
    let args: Vec<String> = std::env::args().collect();
    let mut iter = args.iter().peekable();
    while let Some(arg) = iter.next() {
        if arg == "--config-dir" {
            return iter.next().map(PathBuf::from);
        }
        if let Some(val) = arg.strip_prefix("--config-dir=") {
            return Some(PathBuf::from(val));
        }
    }
    None
}

// Set an env var from a config file if the env var is not already set.
// Additionally, a `$NAME_SOURCE` variant will be set, which allows
// `stellar env` to properly identity the source.
fn set_env_value_from_config<T: std::fmt::Display>(name: &str, value: Option<T>) {
    let Some(value) = value else {
        return;
    };

    std::env::remove_var(format!("{name}_SOURCE"));

    if std::env::var(name).is_err() {
        std::env::set_var(name, value.to_string());
        std::env::set_var(format!("{name}_SOURCE"), "use");
    }
}
