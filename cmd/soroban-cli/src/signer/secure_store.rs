use sep5::SeedPhrase;
use stellar_strkey::ed25519::PublicKey;

use crate::print::Print;

#[cfg(feature = "additional-libs")]
use crate::signer::keyring::{self, StellarEntry};

pub(crate) const ENTRY_PREFIX: &str = "secure_store:";

pub use secure_store_impl::*;

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

#[cfg(feature = "additional-libs")]
mod secure_store_impl {
    use super::*;
    const ENTRY_SERVICE: &str = "org.stellar.cli";

    pub fn get_public_key(
        entry_name: &str,
        index: Option<usize>,
    ) -> Result<PublicKey, Error> {
        let entry = StellarEntry::new(entry_name)?;
        Ok(entry.get_public_key(index)?)
    }
    
    pub fn delete_secret(
        print: &Print,
        entry_name: &str,
    ) -> Result<(), Error> {
        let entry = StellarEntry::new(entry_name)?;
        entry.delete_seed_phrase(print)?;
    
        Ok(())
    }
    
    pub fn save_secret(
        print: &Print,
        entry_name: &str,
        seed_phrase: SeedPhrase,
    ) -> Result<String, Error> {
        // secure_store:org.stellar.cli:<key name>
        let entry_name_with_prefix = format!(
            "{}{}-{}",
            ENTRY_PREFIX,
            ENTRY_SERVICE,
            entry_name
        );

        //checking that the entry name is valid before writing to the secure store
        // let secret = entry_name_with_prefix.parse()?;
        // without this, we end up saving to the keychain without verifying that it is a valid secret name. FIXME

        let entry = StellarEntry::new(&entry_name_with_prefix)?;
        entry.write(seed_phrase, print)?;

        return Ok(entry_name_with_prefix)
    }
    
    pub fn sign_tx_data(
        entry_name: &str,
        hd_path: Option<usize>,
        data: &[u8],
    ) -> Result<Vec<u8>, Error> {
        let entry = StellarEntry::new(entry_name)?;
        return Ok(entry.sign_data(data, hd_path)?)
    }
}

#[cfg(not(feature = "additional-libs"))]
mod secure_store_impl {
    use super::*;

    pub fn get_public_key(
        _entry_name: &str,
        _index: Option<usize>,
    ) -> Result<PublicKey, Error> {
        return Err(Error::FeatureNotEnabled);
    }
    
    pub fn delete_secret(
        _print: &Print,
        _entry_name: &str,
    ) -> Result<(), Error> {
        return Err(Error::FeatureNotEnabled);
    }
    
    pub fn save_secret(
        _print: &Print,
        _entry_name: &str,
        _seed_phrase: SeedPhrase,
    ) -> Result<String, Error> {
        return Err(Error::FeatureNotEnabled);
    }
    
    pub fn sign_tx_data(
        _entry_name: &str,
        _hd_path: Option<usize>,
        _data: &[u8],
    ) -> Result<Vec<u8>, Error> {
        return Err(Error::FeatureNotEnabled);
    }
}

