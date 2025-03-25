use clap::{arg, CommandFactory, Parser};
use clap_complete;
use std::io;

pub const LONG_ABOUT: &str = "\
Print shell completion code for the specified shell

Ensure the completion package for your shell is installed, e.g. bash-completion for bash.

To enable autocomplete in the current bash shell, run: `source <(stellar completion --shell bash)`

To enable autocomplete permanently, run: `echo \"source <(stellar completion --shell bash)\" >> ~/.bashrc`
";

#[derive(Parser, Debug)]
#[group(skip)]
pub struct Cmd {
    /// The shell type
    #[arg(value_enum)]
    shell: ShellType,
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum ShellType {
    Bash,
    Fish,
    Zsh,
    PowerShell,
    Elvish,
}

impl From<ShellType> for clap_complete::Shell {
    fn from(shell: ShellType) -> Self {
        match shell {
            ShellType::Bash => Self::Bash,
            ShellType::Fish => Self::Fish,
            ShellType::Zsh => Self::Zsh,
            ShellType::PowerShell => Self::PowerShell,
            ShellType::Elvish => Self::Elvish,
        }
    }
}

impl Cmd {
    pub fn run(&self) -> Result<(), io::Error> {
        let shell: clap_complete::Shell = self.shell.clone().into();
        clap_complete::generate(
            shell,
            &mut crate::cli::Cli::command(),
            "stellar",
            &mut io::stdout(),
        );
        Ok(())
    }
}
