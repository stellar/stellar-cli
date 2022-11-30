#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("missing default")]
    BadConfig {},
}

#[derive(Debug, clap::Args)]
pub struct Cmd {
    /// List global identities
    #[clap(long)]
    pub global: bool,
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        println!("{self:#?}");
        Ok(())
    }
}
