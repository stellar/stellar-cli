mod migrate;

use clap::Parser;

/// Migrate config from previous versions.
#[derive(Debug, Parser)]
pub enum Cmd {
    /// Migrate the local configuration to the global directory.
    ///
    /// The location will depend on how your system is configured.
    ///
    /// - It looks up for `XDG_CONFIG_HOME` environment variable.
    /// - If not set, it defaults to `$HOME/.config`.
    /// - Can be overridden by `--config-dir` flag.
    Migrate(migrate::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Migrate(#[from] migrate::Error),
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        match self {
            Cmd::Migrate(cmd) => cmd.run()?,
        }
        Ok(())
    }
}
