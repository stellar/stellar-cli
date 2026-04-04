use std::io::{self, BufRead, IsTerminal};

use crate::commands::global;
use crate::config::address::KeyName;
use crate::print::Print;

use super::super::config::locator;

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
            let print = Print::new(global_args.quiet);
            let stdin = io::stdin();

            // Check that the key exists before asking for confirmation
            self.config.read_identity(&self.name)?;

            // Show the prompt only when the user can see it
            if stdin.is_terminal() {
                print.warnln(format!(
                    "Are you sure you want to remove the key '{}'? This action cannot be undone. (y/N)",
                    self.name
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
