mod dir;
mod migrate;

use clap::Parser;

/// Manage CLI configuration.
///
/// Precedence order for CLI configuration:
/// 1. Command-line options & flags (e.g. `--network`, `--source`)
/// 2. Environment variables (e.g. `STELLAR_NETWORK`, `STELLAR_ACCOUNT`)
/// 3. CLI configuration defaults (set via `stellar network/keys/fees use` or `config.toml`)
/// 4. Built-in defaults
#[derive(Debug, Parser)]
pub enum Cmd {
    /// Migrate the local configuration to the global directory.
    Migrate(migrate::Cmd),

    /// Show the global configuration directory.
    ///
    /// The location will depend on how your system is configured.
    ///
    /// - It looks up for `XDG_CONFIG_HOME` environment variable. If it's set,
    ///   `$XDG_CONFIG_HOME/stellar` will be used.
    /// - If not set, it defaults to `$HOME/.config`.
    /// - Can be overridden by `--config-dir` flag.
    Dir(dir::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Migrate(#[from] migrate::Error),

    #[error(transparent)]
    Dir(#[from] dir::Error),
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        match self {
            Cmd::Migrate(cmd) => cmd.run()?,
            Cmd::Dir(cmd) => cmd.run()?,
        }
        Ok(())
    }
}
