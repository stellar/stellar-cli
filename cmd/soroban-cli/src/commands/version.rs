use clap::Parser;
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
    option_env!("GIT_REVISION").unwrap_or("unknown")
}

pub fn long() -> String {
    format!(
        "{} ({})",
        pkg(),
        git()
    )
}
