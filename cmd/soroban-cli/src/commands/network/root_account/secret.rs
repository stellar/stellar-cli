use stellar_strkey::ed25519::PrivateKey;

use super::{Args, Error};

#[derive(Debug, clap::Parser, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub args: Args,
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        let root_key = self.args.root_key()?;
        let private_key = PrivateKey::from_payload(&root_key)?;
        println!("{private_key}");
        Ok(())
    }
}
