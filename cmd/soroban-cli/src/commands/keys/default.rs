use crate::{commands::global, config::locator, print::Print};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Config(#[from] locator::Error),

    #[error("Identify name is required unless --clear is specified")]
    NameRequired,

    #[error("Identify name cannot be used with --clear")]
    NameWithClear,
}

#[derive(Debug, clap::Parser, Clone)]
#[group(skip)]
pub struct Cmd {
    /// Set the default network name.
    pub name: Option<String>,

    /// Clear the default source account.
    #[arg(long)]
    pub clear: bool,

    #[command(flatten)]
    pub config_locator: locator::Args,
}

impl Cmd {
    pub fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let printer = Print::new(global_args.quiet);

        if self.clear && self.name.is_some() {
            return Err(Error::NameWithClear);
        }

        if !self.clear && self.name.is_none() {
            return Err(Error::NameRequired);
        }

        if self.clear {
            self.config_locator.clear_default_identity()?;
            printer.infoln("The default source account has been cleared".to_string());
        } else {
            let name = self.name.as_ref().unwrap();
            let _ = self.config_locator.read_identity(name)?;

            self.config_locator.write_default_identity(name)?;

            printer.infoln(format!("The default source account is set to `{name}`"));
        }

        Ok(())
    }
}
