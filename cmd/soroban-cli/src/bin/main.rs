use clap::{CommandFactory, Parser};
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

    if let Err(e) = root.run().await {
        eprintln!("error: {e}");
    }
}
