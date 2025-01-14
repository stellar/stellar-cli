use sep5::SeedPhrase;

use crate::{
    config::{address::KeyName, locator, secret::{self, Secret}}, print::Print, signer::keyring::{self, StellarEntry}
};

pub struct SecureStore {}

#[derive(thiserror::Error, Debug)]
pub enum Error{
    #[error(transparent)]
    Config(#[from] locator::Error),

    #[error(transparent)]
    Secret(#[from] secret::Error),

    #[error(transparent)]
    Keyring(#[from] keyring::Error),

    #[error("Storing an existing private key in Secure Store is not supported")]
    DoesNotSupportPrivateKey,

    #[error(transparent)]
    SeedPhrase(#[from] sep5::Error)
}

impl SecureStore {
    pub fn save_secret(print: &Print, entry_name: &KeyName, seed_phrase: SeedPhrase) -> Result<Secret, Error> {
        // secure_store:org.stellar.cli:<key name>
        let entry_name_with_prefix = format!(
            "{}{}-{}",
            keyring::SECURE_STORE_ENTRY_PREFIX,
            keyring::SECURE_STORE_ENTRY_SERVICE,
            entry_name
        );

        //checking that the entry name is valid before writing to the secure store
        let secret: Secret = entry_name_with_prefix.parse()?;

        if let Secret::SecureStore { entry_name } = &secret {
            Self::write_to_secure_store(entry_name, seed_phrase, print)?;
        }

        return Ok(secret);
    }

    fn write_to_secure_store(
        entry_name: &String,
        seed_phrase: SeedPhrase,
        print: &Print,
    ) -> Result<(), Error> {
        print.infoln(format!("Writing to secure store: {entry_name}"));
        let entry = StellarEntry::new(entry_name)?;
        Ok(if let Ok(key) = entry.get_public_key(None) {
            print.warnln(format!("A key for {entry_name} already exists in your operating system's secure store: {key}"));
        } else {
            print.infoln(format!(
                "Saving a new key to your operating system's secure store: {entry_name}"
            ));
            entry.set_seed_phrase(seed_phrase)?;
        })
    }
}