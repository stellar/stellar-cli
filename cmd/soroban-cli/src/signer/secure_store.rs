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
    use super::{Error, Print, PublicKey, SeedPhrase, StellarEntry, ENTRY_PREFIX};
    const ENTRY_SERVICE: &str = "org.stellar.cli";

    pub fn get_public_key(entry_name: &str, index: Option<usize>) -> Result<PublicKey, Error> {
        let entry = StellarEntry::new(entry_name)?;
        Ok(entry.get_public_key(index)?)
    }

    pub fn get_public_key_with_entry(entry: &StellarEntry, index: Option<usize>) -> Result<PublicKey, Error> {
        Ok(entry.get_public_key(index)?)
    }

    pub fn delete_secret(print: &Print, entry_name: &str) -> Result<(), Error> {
        let entry = StellarEntry::new(entry_name)?;
        Ok(entry.delete_seed_phrase(print)?)
    }

    pub fn save_secret(
        print: &Print,
        entry_name: &str,
        seed_phrase: &SeedPhrase,
    ) -> Result<String, Error> {
        // secure_store:org.stellar.cli:<key name>
        let entry_name_with_prefix = format!("{ENTRY_PREFIX}{ENTRY_SERVICE}-{entry_name}");

        let entry = StellarEntry::new(&entry_name_with_prefix)?;
        entry.write(seed_phrase.clone(), print)?;

        Ok(entry_name_with_prefix)
    }

    pub fn sign_tx_data(
        entry_name: &str,
        hd_path: Option<usize>,
        data: &[u8],
    ) -> Result<Vec<u8>, Error> {
        let entry = StellarEntry::new(entry_name)?;
        Ok(entry.sign_data(data, hd_path)?)
    }

    pub fn sign_tx_data_with_entry(
        entry: &StellarEntry,
        hd_path: Option<usize>,
        data: &[u8],
    ) -> Result<Vec<u8>, Error> {
        Ok(entry.sign_data(data, hd_path)?)
    }
}

#[cfg(not(feature = "additional-libs"))]
mod secure_store_impl {
    use super::{Error, Print, PublicKey, SeedPhrase};

    pub fn get_public_key(_entry_name: &str, _index: Option<usize>) -> Result<PublicKey, Error> {
        Err(Error::FeatureNotEnabled)
    }

    pub fn delete_secret(_print: &Print, _entry_name: &str) -> Result<(), Error> {
        Err(Error::FeatureNotEnabled)
    }

    pub fn save_secret(
        _print: &Print,
        _entry_name: &str,
        _seed_phrase: &SeedPhrase,
    ) -> Result<String, Error> {
        Err(Error::FeatureNotEnabled)
    }

    pub fn sign_tx_data(
        _entry_name: &str,
        _hd_path: Option<usize>,
        _data: &[u8],
    ) -> Result<Vec<u8>, Error> {
        Err(Error::FeatureNotEnabled)
    }
}
