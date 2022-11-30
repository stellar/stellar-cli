#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("No such idenity: {name}")]
    NoSuchIdentity { name: String },
}

#[derive(Debug, clap::Args)]
pub struct Cmd {
    /// alias to remove
    pub alias: String,
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        println!("{self:#?}");
        Ok(())
    }
}
