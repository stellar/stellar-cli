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

pub fn short() -> String {
    format!("{} ({GIT_REVISION})", env!("CARGO_PKG_VERSION"))
}

pub fn long() -> String {
    let env = soroban_env_host::VERSION;
    let xdr = soroban_env_host::VERSION.xdr;
    vec![
        short(),
        format!("soroban-env {} ({})", env.pkg, env.rev),
        format!("soroban-env interface version {}", meta::INTERFACE_VERSION),
        format!(
            "stellar-xdr {} ({})
xdr next ({})",
            xdr.pkg, xdr.rev, xdr.xdr_next,
        ),
    ]
    .join("\n")
}

// Check that the XDR cannel in use is 'next' to ensure that the version output
// is not forgotten when we eventually update to using curr. This is a bit of a
// hack because of limits of what you can do in a constant context, but by being
// a constant context this is checked at compile time.
const _: () = {
    #[allow(clippy::single_match)]
    match soroban_env_host::VERSION.xdr.xdr.as_bytes() {
        b"next" => (),
        _ => panic!("xdr version channel needs updating"),
    }
};
