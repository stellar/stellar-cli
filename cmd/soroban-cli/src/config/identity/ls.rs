use crate::config;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Config(#[from] config::location::Error),
}

#[derive(Debug, clap::Args)]
pub struct Cmd {
    #[clap(flatten)]
    pub config: config::location::Args,
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        let res = self.config.list_identities()?;
        println!("{}", res.join("\n"));
        Ok(())
    }
}
