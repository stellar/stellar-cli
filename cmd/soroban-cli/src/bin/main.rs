use clap::{CommandFactory, Parser};
use tracing_subscriber::{fmt, EnvFilter};

use soroban_cli::{commands::plugin, Root};

#[tokio::main]
async fn main() {
    let root = Root::try_parse().unwrap_or_else(|e| {
        use clap::error::ErrorKind;
        match e.kind() {
            ErrorKind::InvalidSubcommand => {
                if let Err(error) = plugin::run() {
                    eprintln!("error: {error}");
                    std::process::exit(1)
                } else {
                    std::process::exit(0)
                }
            }
            ErrorKind::MissingSubcommand if std::env::args().any(|s| &s == "--list") => {
                println!(
                    "Installed Plugins:\n    {}",
                    plugin::list().unwrap_or_default().join("\n    ")
                );
                std::process::exit(0);
            }
            _ => {
                let mut cmd = Root::command();
                e.format(&mut cmd).exit();
            }
        }
    });
    // Now use root to setup the logger
    if let Some(level) = root.global_args.log_level() {
        let filter = EnvFilter::new(format!("soroban_cli={level},hyper=off"));
        let subscriber = fmt::Subscriber::builder()
            .with_env_filter(filter)
            .with_writer(std::io::stderr)
            .finish();
        tracing::subscriber::set_global_default(subscriber)
            .expect("Failed to set the global tracing subscriber");
    }

    if let Err(e) = root.run().await {
        eprintln!("error: {e}");
    }
}
