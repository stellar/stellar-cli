use clap::Parser;
use soroban_env_host::meta;
use std::fmt::Debug;

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd;

impl Cmd {
    #[allow(clippy::unused_self)]
    pub fn run(&self) {
        println!("stellar {}", long());
    }
}

pub fn pkg() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub fn git() -> &'static str {
    env!("GIT_REVISION")
}

pub fn long() -> String {
    let env = soroban_env_host::VERSION;
    let xdr = soroban_env_host::VERSION.xdr;
    [
        format!("{} ({})", pkg(), git()),
        format!("soroban-env {} ({})", env.pkg, env.rev),
        format!(
            "soroban-env protocol version {}",
            meta::INTERFACE_VERSION.protocol
        ),
        (if meta::INTERFACE_VERSION.pre_release == 0 {
            "soroban-env pre-release version n/a".to_string()
        } else {
            format!(
                "soroban-env pre-release version {}",
                meta::INTERFACE_VERSION.pre_release
            )
        }),
        format!(
            "stellar-xdr {} ({})
xdr curr ({})",
            xdr.pkg, xdr.rev, xdr.xdr_curr,
        ),
    ]
    .join("\n")
}
