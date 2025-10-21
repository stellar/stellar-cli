use clap::Parser;
use std::fmt::Debug;

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    /// Print only the version.
    #[arg(long)]
    only_version: bool,
    /// Print only the major version.
    #[arg(long)]
    only_version_major: bool,
}

impl Cmd {
    #[allow(clippy::unused_self)]
    pub fn run(&self) {
        if self.only_version {
            println!("{}", pkg());
        } else if self.only_version_major {
            println!("{}", major());
        } else {
            println!("stellar {}", long());
        }
    }
}

pub fn pkg() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub fn major() -> &'static str {
    env!("CARGO_PKG_VERSION_MAJOR")
}

pub fn git() -> &'static str {
    env!("GIT_REVISION")
}

pub fn long() -> String {
    let xdr = stellar_xdr::VERSION;
    [
        format!("{} ({})", pkg(), git()),
        format!(
            "stellar-xdr {} ({})
xdr curr ({})",
            xdr.pkg, xdr.rev, xdr.xdr_curr,
        ),
    ]
    .join("\n")
}

pub fn one_line() -> String {
    let pkg = pkg();
    let git = git();
    format!("{pkg}#{git}")
}
