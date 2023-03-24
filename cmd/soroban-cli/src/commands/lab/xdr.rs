mod decode;

use std::fmt::Debug;

use clap::{Parser, Subcommand};

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[clap(subcommand)]
    sub: SubCmd,
}

#[derive(Subcommand, Debug, Clone)]
enum SubCmd {
    /// Decode XDR
    Dec(decode::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("decode: {0}")]
    Decode(#[from] decode::Error),
}

impl Cmd {
    #[allow(clippy::too_many_lines)]
    pub fn run(&self) -> Result<(), Error> {
        match &self.sub {
            SubCmd::Dec(d) => d.run()?,
        };
        Ok(())
    }
}
