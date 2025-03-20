use sep5::SeedPhrase;

use crate::print::Print;

#[cfg(feature = "additional-libs")]
use crate::signer::keyring::{self, StellarEntry};

#[derive(thiserror::Error, Debug)]
pub enum Error {
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


pub fn delete_secret(
    print: &Print,
    entry_name: &str,
) -> Result<(), Error> {

    #[cfg(feature = "additional-libs")]
    {
        let entry = StellarEntry::new(entry_name)?;
        entry.delete_seed_phrase(print)?;

        return Ok(())
    }
    return Err(Error::FeatureNotEnabled);
}

pub fn save_secret(
    print: &Print,
    entry_name: &str,
    seed_phrase: SeedPhrase,
) -> Result<String, Error> {

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
        // let secret = entry_name_with_prefix.parse()?;
        // without this, we end up saving to the keychain without verifying that it is a valid secret name. FIXME

        let entry = StellarEntry::new(entry_name)?;
        entry.write(seed_phrase, print)?;

        return Ok(entry_name_with_prefix)
    }
    return Err(Error::FeatureNotEnabled);
}
