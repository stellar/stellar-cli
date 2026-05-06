use clap::{CommandFactory, Parser};
use clap_complete::{generate, Shell};
use std::io;

use crate::commands::Root;

pub const LONG_ABOUT: &str = "\
Print shell completion code for the specified shell

Ensure the completion package for your shell is installed, e.g. bash-completion for bash.

To enable autocomplete in the current bash shell, run: `source <(stellar completion --shell bash)`

To enable autocomplete permanently, run: `echo \"source <(stellar completion --shell bash)\" >> ~/.bashrc`
";

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    /// The shell type
    #[arg(long, value_enum)]
    shell: Shell,
}

impl Cmd {
    pub fn run(&self) {
        let cmd = &mut Root::command();
        generate(self.shell, cmd, "stellar", &mut io::stdout());
    }
}
