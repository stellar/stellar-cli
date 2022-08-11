use clap::{ArgEnum, Command, Parser};
use clap_complete::{generate, Shell};
use std::io;

pub const LONG_ABOUT: &str = "\
Print shell completion code for the specified shell

Ensure the completion package for your shell is installed,
e.g., bash-completion for bash.

To enable autocomplete in the current bash shell, run:
  source <(soroban-cli completion bash)
  
To enable autocomplete permanently, run:
  echo \"source <(soroban-cli completion bash)\" >> ~/.bashrc";

#[derive(Parser, Debug)]
pub struct Cmd {
    /// The shell type
    #[clap(arg_enum, value_parser)]
    shell: ShellType,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ArgEnum, Debug)]
enum ShellType {
    Bash,
    Zsh,
    Fish,
    Elvish,
    #[clap(name = "powershell")]
    PowerShell,
}

impl Cmd {
    pub fn run(&self, cmd: &mut Command) {
        let gen = match self.shell {
            ShellType::Bash => Shell::Bash,
            ShellType::Zsh => Shell::Zsh,
            ShellType::Fish => Shell::Fish,
            ShellType::Elvish => Shell::Elvish,
            ShellType::PowerShell => Shell::PowerShell,
        };

        generate(gen, cmd, env!("CARGO_PKG_NAME"), &mut io::stdout());
    }
}
