use crate::{
    commands::global,
    config::locator::{self},
    env_vars,
    print::Print,
    utils::escape_control_characters,
};
use clap::Parser;
use shell_escape::escape;

#[allow(clippy::doc_markdown)]
#[derive(Debug, Parser)]
pub struct Cmd {
    /// Env variable name to get the value of.
    ///
    /// E.g.: $ stellar env STELLAR_ACCOUNT
    #[arg()]
    pub name: Option<String>,

    /// Whether to reveal the value of concealed env vars. By default, concealed env vars are
    /// hidden behind a placeholder value.
    #[arg(long, default_value_t = false)]
    pub reveal: bool,

    #[command(flatten)]
    pub config_locator: locator::Args,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Locator(#[from] locator::Error),
}

impl Cmd {
    pub fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let print = Print::new(global_args.quiet);
        let mut vars: Vec<EnvVar> = Vec::new();
        let supported = env_vars::prefixed("STELLAR");

        for key in supported {
            if let Some(mut v) = EnvVar::get(&key) {
                if self.reveal {
                    v.reveal();
                }

                vars.push(v);
            }
        }

        // If a specific name is given, just print that one value
        if let Some(name) = &self.name {
            if let Some(v) = vars.iter().find(|v| &v.key == name) {
                if v.is_revealed() {
                    println!("{}", escape_control_characters(&v.value));
                }
            }

            return Ok(());
        }

        if vars.is_empty() {
            print.warnln("No defaults or environment variables set".to_string());
            return Ok(());
        }

        let max_len = vars.iter().map(|v| v.str().len()).max().unwrap_or(0);

        vars.sort();

        for v in vars {
            println!("{:max_len$} # {}", v.str(), v.source);
        }

        Ok(())
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct EnvVar {
    key: String,
    value: String,
    source: String,
    reveal: bool,
}

impl EnvVar {
    fn get(key: &str) -> Option<Self> {
        // The _SOURCE env var is set from cmd/soroban-cli/src/cli.rs#set_env_value_from_config
        let source = std::env::var(format!("{key}_SOURCE"))
            .ok()
            .unwrap_or("env".to_string());

        if let Ok(value) = std::env::var(key) {
            return Some(EnvVar {
                key: key.to_string(),
                value,
                source,
                reveal: false,
            });
        }

        None
    }

    fn reveal(&mut self) {
        self.reveal = true;
    }

    fn is_revealed(&self) -> bool {
        self.reveal || !env_vars::is_concealed(&self.key)
    }

    fn str(&self) -> String {
        if self.is_revealed() {
            let value = escape(escape_control_characters(&self.value).into());
            format!("{}={value}", self.key)
        } else {
            format!("# {}=<concealed>", self.key)
        }
    }
}
