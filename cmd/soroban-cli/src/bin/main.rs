use clap::{CommandFactory, Parser};
use soroban_cli::{commands::custom::run_external, Root};

#[tokio::main]
async fn main() {
    let root = Root::try_parse().unwrap_or_else(|e| {
        if let clap::error::ErrorKind::UnknownArgument | clap::error::ErrorKind::InvalidSubcommand =
            e.kind()
        {
            run_external().unwrap();
            e.exit();
        } else {
            let mut cmd = Root::command();
            e.format(&mut cmd).exit();
        }
    });

    if let Err(e) = root.run().await {
        eprintln!("error: {e}");
    }
}
