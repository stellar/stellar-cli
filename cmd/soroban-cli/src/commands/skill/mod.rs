use clap::Parser;

pub const LONG_ABOUT: &str = "\
Print an AI-agent skill guide for using the Stellar CLI

Outputs a Markdown document describing how to use the Stellar CLI idiomatically.
It is meant to be read by AI coding agents (or pasted into their instructions)
so they follow the CLI's conventions: using named networks, identities, and
contract aliases instead of raw RPC URLs, secret keys, and hard-coded contract
ids.
";

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {}

impl Cmd {
    #[allow(clippy::unused_self)]
    pub fn run(&self) {
        print!("{}", include_str!("SKILL.md"));
    }
}
