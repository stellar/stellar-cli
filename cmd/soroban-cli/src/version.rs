use clap::Parser;
use soroban_env_host::meta;
use std::fmt::Debug;

const GIT_REVISION: &str = env!("GIT_REVISION");

#[derive(Parser, Debug)]
pub struct Cmd;

impl Cmd {
    #[allow(clippy::unused_self)]
    pub fn run(&self) {
        println!("soroban {} ({})", env!("CARGO_PKG_VERSION"), GIT_REVISION,);
        println!("soroban-env-interface-version: {}", meta::INTERFACE_VERSION);
    }
}
