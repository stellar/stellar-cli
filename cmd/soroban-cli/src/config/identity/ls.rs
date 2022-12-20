use crate::config::location;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Config(#[from] location::Error),
}

#[derive(Debug, clap::Args)]
pub struct Cmd {
    #[clap(flatten)]
    pub config: location::Args,
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        let res = self.config.list_identities()?;
        println!("{}", res.join("\n"));
        Ok(())
    }
}
