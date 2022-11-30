#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("missing default")]
    MissingDefault {},
}

#[derive(Debug, clap::Args)]
pub struct Cmd {
    /// default alais
    pub default_alias: String,
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        println!("{self:#?}");
        Ok(())
    }
}
