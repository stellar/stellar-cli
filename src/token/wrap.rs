use std::fmt::Debug;

use clap::Parser;

#[derive(thiserror::Error, Debug)]
pub enum Error {}

#[derive(Parser, Debug)]
pub struct Cmd {
    /// ID of the Stellar classic asset to wrap, e.g. "USDC:G...5"
    #[clap(long)]
    asset: String,
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        Ok(())
    }
}
