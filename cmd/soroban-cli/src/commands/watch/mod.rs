mod app;
mod decode;
mod error;
mod event;
mod filter;
mod sources;
mod spec_cache;
mod tui;
mod ui;

use clap::Parser;
use tokio::sync::mpsc;

use crate::config::{locator, network};

#[derive(Parser, Debug, Clone)]
#[command(about = "Live TUI dashboard for Soroban RPC events and transactions")]
pub struct Cmd {
    #[command(flatten)]
    pub locator: locator::Args,

    #[command(flatten)]
    pub network: network::Args,

    /// Filter by public key (repeatable)
    #[arg(short = 'a', long = "address")]
    pub addresses: Vec<String>,

    /// Filter by asset code or contract ID (repeatable)
    #[arg(short = 't', long = "token")]
    pub tokens: Vec<String>,

    /// Filter by event/operation type (repeatable)
    #[arg(short = 'e', long = "event-type")]
    pub event_types: Vec<String>,

    /// RPC poll interval in seconds
    #[arg(long, default_value = "5")]
    pub poll_interval: u64,

    /// Max events to keep in memory
    #[arg(long, default_value = "1000")]
    pub max_events: usize,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Network(#[from] network::Error),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Other(String),
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        let network_config = self.network.get(&self.locator)?;
        let rpc_url = network_config.rpc_url.clone();
        let network_passphrase = network_config.network_passphrase.clone();

        let (tx, rx) = mpsc::unbounded_channel();

        let mut app = app::App::new(self.max_events);
        app.network = network_name_from_passphrase(&network_passphrase);
        app.network_passphrase.clone_from(&network_passphrase);
        app.filter.addresses.clone_from(&self.addresses);
        app.filter.tokens.clone_from(&self.tokens);
        app.filter.event_types.clone_from(&self.event_types);

        // Suppress panic messages to stderr for the duration of the TUI session.
        // soroban_spec_tools hits todo!() for some ScVal types; we catch those with
        // catch_unwind, but the default panic hook would still print to the terminal
        // and corrupt the TUI display.
        let prev_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|info| {
            tracing::debug!("panic suppressed during TUI: {info}");
        }));

        let terminal = tui::setup_terminal()?;
        let result = tui::run_event_loop(
            terminal,
            app,
            rx,
            tx,
            rpc_url,
            self.poll_interval,
            network_passphrase,
            self.locator.clone(),
            self.network.clone(),
        );

        std::panic::set_hook(prev_hook);
        result.map_err(|e| Error::Other(e.to_string()))?;

        Ok(())
    }
}

fn network_name_from_passphrase(passphrase: &str) -> String {
    use crate::config::network::passphrase as p;
    match passphrase {
        p::MAINNET => "mainnet",
        p::TESTNET => "testnet",
        p::FUTURENET => "futurenet",
        p::LOCAL => "local",
        _ => "unknown",
    }
    .to_string()
}
