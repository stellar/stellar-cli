use clap::Parser;
use soroban_env_host::meta;
use std::fmt::Debug;

#[derive(Parser, Debug)]
pub struct Cmd;

impl Cmd {
    #[allow(clippy::unused_self)]
    pub fn run(&self) {
        println!("soroban-cli {}", env!("CARGO_PKG_VERSION"),);
        println!("soroban-env interface {}", meta::INTERFACE_VERSION);
    }
}
