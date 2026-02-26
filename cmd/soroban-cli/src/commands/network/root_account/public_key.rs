use stellar_strkey::ed25519::PublicKey;

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
        let signing_key = ed25519_dalek::SigningKey::from_bytes(&root_key);
        let public_key = PublicKey::from_payload(signing_key.verifying_key().as_bytes())?;
        println!("{public_key}");
        Ok(())
    }
}
