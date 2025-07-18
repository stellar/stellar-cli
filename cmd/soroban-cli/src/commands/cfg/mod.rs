mod migrate;
mod show;

use clap::Parser;

/// Migrate config from previous versions.
#[derive(Debug, Parser)]
pub enum Cmd {
    /// Migrate the local configuration to the global directory.
    Migrate(migrate::Cmd),

    /// Show the global configuration directory.
    ///
    /// The location will depend on how your system is configured.
    ///
    /// - It looks up for `XDG_CONFIG_HOME` environment variable. If it's set,
    ///  `$XDG_CONFIG_HOME/stellar` will be used.
    /// - If not set, it defaults to `$HOME/.config`.
    /// - Can be overridden by `--config-dir` flag.
    Show(show::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Migrate(#[from] migrate::Error),

    #[error(transparent)]
    Show(#[from] show::Error),
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        match self {
            Cmd::Migrate(cmd) => cmd.run()?,
            Cmd::Show(cmd) => cmd.run()?,
        }
        Ok(())
    }
}
