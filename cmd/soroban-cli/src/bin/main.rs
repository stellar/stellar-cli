use clap::CommandFactory;
use soroban_cli::Root;

#[tokio::main]
async fn main() {
    let root = Root::new().unwrap_or_else(|e| {
        let mut cmd = Root::command();
        e.format(&mut cmd).exit();
    });

    if let Err(e) = root.run().await {
        eprintln!("error: {e}");
    }
}
