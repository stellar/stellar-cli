use sep5::SeedPhrase;

use crate::{
    config::{
        address::KeyName,
        locator,
        secret::{self, Secret},
    },
    print::Print,
};

#[cfg(feature = "additional-libs")]
use crate::signer::keyring::{self, StellarEntry};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Config(#[from] locator::Error),

    #[error(transparent)]
    Secret(#[from] secret::Error),

    #[cfg(feature = "additional-libs")]
    #[error(transparent)]
    Keyring(#[from] keyring::Error),

    #[error("Storing an existing private key in Secure Store is not supported")]
    DoesNotSupportPrivateKey,

    #[error(transparent)]
    SeedPhrase(#[from] sep5::Error),

    #[error("Secure Store keys are not allowed: additional-libs feature must be enabled")]
    FeatureNotEnabled,
}

pub fn save_secret(
    print: &Print,
    entry_name: &KeyName,
    seed_phrase: SeedPhrase,
) -> Result<Secret, Error> {

    #[cfg(feature = "additional-libs")]
    {
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
            let entry = StellarEntry::new(entry_name)?;
            entry.write(seed_phrase, print)?;
        }

        return Ok(secret)
    }
    return Err(Error::FeatureNotEnabled);

}
