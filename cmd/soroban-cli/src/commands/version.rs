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
    let git_rev = if git().is_empty() {
        String::new()
    } else {
        format!(" ({})", git())
    };

    [
        format!("{}{git_rev}", pkg()),
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

#[test]
fn test_long_without_git_rev() {
    std::env::remove_var("GIT_REVISION");
    let expected = format!(
        "{}\nstellar-xdr {} ({})\nxdr curr ({})",
        pkg(),
        stellar_xdr::VERSION.pkg,
        stellar_xdr::VERSION.rev,
        stellar_xdr::VERSION.xdr_curr,
    );
    assert_eq!(long(), expected);
}

#[test]
fn test_long_with_git_rev() {
    std::env::set_var("GIT_REVISION", "REF");
    let expected = format!(
        "{} (REF)\nstellar-xdr {} ({})\nxdr curr ({})",
        pkg(),
        stellar_xdr::VERSION.pkg,
        stellar_xdr::VERSION.rev,
        stellar_xdr::VERSION.xdr_curr,
    );
    assert_eq!(long(), expected);
    std::env::remove_var("GIT_REVISION");
}
