use std::io::{self, BufRead, IsTerminal};

use crate::commands::global;
use crate::config::address::KeyName;
use crate::config::locator::{self, KeyType};
use crate::print::Print;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Locator(#[from] locator::Error),
    #[error("removal cancelled by user")]
    Cancelled,
    #[error(transparent)]
    Io(#[from] io::Error),
}

#[derive(Debug, clap::Parser, Clone)]
#[group(skip)]
pub struct Cmd {
    /// Identity to remove
    pub name: KeyName,

    /// Skip confirmation prompt
    #[arg(long)]
    pub force: bool,

    #[command(flatten)]
    pub config: locator::Args,
}

impl Cmd {
    pub fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        if !self.force {
            let print = Print::new(false);
            let stdin = io::stdin();

            // Check that the key exists before asking for confirmation
            self.config.read_identity(&self.name)?;

            // Show the prompt only when the user can see it
            if stdin.is_terminal() {
                let config_path = KeyType::Identity.path(&self.config.config_dir()?, &self.name);
                print.warnln(format!(
                    "Are you sure you want to remove the key '{}' at '{}'? This action cannot be undone. (y/N)",
                    self.name,
                    config_path.display()
                ));
            }
            let mut response = String::new();
            stdin.lock().read_line(&mut response)?;
            if !response.trim().eq_ignore_ascii_case("y") {
                return Err(Error::Cancelled);
            }
        }
        Ok(self.config.remove_identity(&self.name, global_args)?)
    }
}
