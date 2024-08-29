use clap::CommandFactory;
use dotenvy::dotenv;
use std::thread;
use tracing_subscriber::{fmt, EnvFilter};

use crate::self_outdated_check::print_upgrade_prompt;
use crate::{commands, Root};

#[tokio::main]
pub async fn main() {
    // Spawn a thread to print the upgrade prompt in the background
    thread::spawn(print_upgrade_prompt);

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

    if let Err(e) = root.run().await {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
