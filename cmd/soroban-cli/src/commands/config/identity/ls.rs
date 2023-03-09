use super::super::locator;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Config(#[from] locator::Error),
}

#[derive(Debug, clap::Args)]
pub struct Cmd {
    #[clap(flatten)]
    pub config_locator: locator::Args,
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        let res = self.config_locator.list_identities()?;
        println!("{}", res.join("\n"));
        Ok(())
    }
}
