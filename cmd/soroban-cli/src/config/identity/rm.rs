#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("No such idenity: {name}")]
    NoSuchIdentity { name: String },
}

#[derive(Debug, clap::Args)]
pub struct Cmd {
    /// alias to associate with the profile
    pub alias: String,

    /// Add using secret_key
    #[clap(long)]
    pub secret_key: Option<bool>,

    /// Add using 12 word seed phrase to generate secret_key
    #[clap(long)]
    pub seed_phrase: Option<bool>,

    /// Generate a new key pair and print seed phrase
    #[clap(long)]
    pub generate: Option<bool>,
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        println!("{self:#?}");
        Ok(())
    }
}
