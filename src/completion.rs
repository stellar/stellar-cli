use clap::{Command, Parser};
use clap_complete::{generate, Shell};
use std::io;

pub const LONG_ABOUT: &str = "\
Print shell completion code for the specified shell

Ensure the completion package for your shell is installed,
e.g., bash-completion for bash.

To enable autocomplete in the current bash shell, run:
  source <(soroban-cli completion --shell bash)

To enable autocomplete permanently, run:
  echo \"source <(soroban-cli completion --shell bash)\" >> ~/.bashrc";

#[derive(Parser, Debug)]
pub struct Cmd {
    /// The shell type
    #[clap(long, arg_enum)]
    shell: Shell,
}

impl Cmd {
    pub fn run(&self, cmd: &mut Command) {
        generate(self.shell, cmd, env!("CARGO_PKG_NAME"), &mut io::stdout());
    }
}
