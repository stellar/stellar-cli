#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("No such identity {name}")]
    MissingIdentity { name: String },
}

#[derive(Debug, clap::Args)]
pub struct Cmd {
    /// default name
    pub default_name: String,
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        println!("{self:#?}");
        Ok(())
    }
}
