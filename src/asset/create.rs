use std::fmt::Debug;

use clap::Parser;

#[derive(thiserror::Error, Debug)]
pub enum Error {
}

#[derive(Parser, Debug)]
pub struct Cmd {
    /// Administrator account for the asset
    /// TODO: Do we need this? Or use source of deployer?
    #[clap(long)]
    admin: String,

    /// Number of decimal places for the asset
    #[clap(long, default_value = "7")]
    decimal: u32,

    /// Long name of the asset, e.g. "Stellar Lumens"
    #[clap(long)]
    name: String,

    /// Short name of the asset, e.g. "XLM"
    #[clap(long)]
    symbol: String,
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        Ok(())
    }
}
