use clap::Parser;
use std::fmt::Debug;
use stellar_contract_env_host::meta;

#[derive(Parser, Debug)]
pub struct Cmd;

impl Cmd {
    #[allow(clippy::unused_self)]
    pub fn run(&self) {
        println!("stellar-contract-cli {}", env!("CARGO_PKG_VERSION"),);
        println!("stellar-contract-env interface {}", meta::INTERFACE_VERSION);
    }
}
