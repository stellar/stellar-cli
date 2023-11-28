use clap::Parser;
use soroban_env_host::meta;
use std::fmt::Debug;

const GIT_REVISION: &str = env!("GIT_REVISION");

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd;

impl Cmd {
    #[allow(clippy::unused_self)]
    pub fn run(&self) {
        println!("soroban {}", long());
    }
}

pub fn long() -> String {
    let env = soroban_env_host::VERSION;
    let xdr = soroban_env_host::VERSION.xdr;
    [
        format!("{} ({GIT_REVISION})", env!("CARGO_PKG_VERSION")),
        format!("soroban-env {} ({})", env.pkg, env.rev),
        format!("soroban-env interface version {}", meta::INTERFACE_VERSION),
        format!(
            "stellar-xdr {} ({})
xdr curr ({})",
            xdr.pkg, xdr.rev, xdr.xdr_curr,
        ),
    ]
    .join("\n")
}
