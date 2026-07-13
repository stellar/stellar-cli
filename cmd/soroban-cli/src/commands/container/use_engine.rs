use crate::{commands::global, config::locator, print::Print};

use super::shared::Engine;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Config(#[from] locator::Error),
}

#[derive(Debug, clap::Parser, Clone)]
#[group(skip)]
pub struct Cmd {
    /// Container engine to use by default.
    pub engine: Engine,

    #[command(flatten)]
    pub config_locator: locator::Args,
}

impl Cmd {
    pub fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let print = Print::new(global_args.quiet);

        if std::env::var("STELLAR_CONTAINER_ENGINE").is_ok()
            && std::env::var("STELLAR_CONTAINER_ENGINE_SOURCE").is_err()
        {
            print.warnln("Environment variable STELLAR_CONTAINER_ENGINE is set, which will override this default engine.");
        }

        self.config_locator
            .write_default_container_engine(&self.engine.to_string())?;

        print.infoln(format!(
            "The default container engine is set to `{}`",
            self.engine
        ));

        Ok(())
    }
}
