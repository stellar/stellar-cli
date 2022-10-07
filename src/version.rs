use clap::Parser;
use soroban_env_host::meta;
use std::fmt::Debug;

const GIT_SHA: Option<&str> = option_env!("GIT_SHA");

#[derive(Parser, Debug)]
pub struct Cmd;

impl Cmd {
    #[allow(clippy::unused_self)]
    pub fn run(&self) {
        println!(
            "soroban-cli {} ({})",
            env!("CARGO_PKG_VERSION"),
            GIT_SHA.unwrap_or_default(),
        );
        println!("soroban-env-interface-version: {}", meta::INTERFACE_VERSION);
    }
}
