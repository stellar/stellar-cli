use clap::{CommandFactory, Parser};
use tracing_subscriber::{fmt, EnvFilter};

use soroban_cli::{commands::plugin, Root};

#[tokio::main]
async fn main() {
    let mut root = Root::try_parse().unwrap_or_else(|e| {
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
        let mut e_filter = EnvFilter::from_default_env()
            .add_directive("hyper=off".parse().unwrap())
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
            .with_writer(std::io::stderr);

        let subscriber = builder.finish();
        tracing::subscriber::set_global_default(subscriber)
            .expect("Failed to set the global tracing subscriber");
    }

    if let Err(e) = root.run().await {
        eprintln!("error: {e}");
    }
}
