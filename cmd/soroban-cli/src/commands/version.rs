use clap::Parser;
use soroban_env_host::meta;
use std::fmt::Debug;

use crate::xdr::ScEnvMetaEntryInterfaceVersion;

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
    let ScEnvMetaEntryInterfaceVersion {
        protocol,
        pre_release,
    } = meta::INTERFACE_VERSION;
    [
        format!("{} ({})", pkg(), git()),
        format!("soroban-env {} ({})", env.pkg, env.rev),
        format!("soroban-env interface version and prerelease {protocol}, {pre_release}"),
        format!(
            "stellar-xdr {} ({})
xdr curr ({})",
            xdr.pkg, xdr.rev, xdr.xdr_curr,
        ),
    ]
    .join("\n")
}
