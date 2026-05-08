use sep5::SeedPhrase;
use stellar_strkey::ed25519::PublicKey;

use crate::config::secret::Secret;
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
    use super::{Error, Print, PublicKey, Secret, SeedPhrase, StellarEntry, ENTRY_PREFIX};
    const ENTRY_SERVICE: &str = "org.stellar.cli";

    pub fn get_public_key(entry_name: &str, index: Option<u32>) -> Result<PublicKey, Error> {
        let entry = StellarEntry::new(entry_name)?;
        Ok(entry.get_public_key(index)?)
    }

    pub fn delete_secret(print: &Print, entry_name: &str) -> Result<(), Error> {
        let entry = StellarEntry::new(entry_name)?;
        Ok(entry.delete_seed_phrase(print)?)
    }

    pub fn save_secret(
        print: &Print,
        name: &str,
        seed_phrase: &SeedPhrase,
        hd_path: Option<u32>,
        overwrite: bool,
    ) -> Result<Secret, Error> {
        // secure_store:org.stellar.cli-<key name>
        let entry_name = format!("{ENTRY_PREFIX}{ENTRY_SERVICE}-{name}");

        let entry = StellarEntry::new(&entry_name)?;
        entry.write(seed_phrase.clone(), print, overwrite)?;

        let public_key_bytes = seed_phrase
            .clone()
            .from_path_index(hd_path.unwrap_or_default() as usize, None)?
            .public()
            .0;
        let public_key = PublicKey(public_key_bytes).to_string();

        Ok(Secret::SecureStore {
            entry_name,
            public_key: Some(public_key),
            hd_path,
        })
    }

    pub fn sign_tx_data(
        entry_name: &str,
        hd_path: Option<u32>,
        data: &[u8],
    ) -> Result<Vec<u8>, Error> {
        let entry = StellarEntry::new(entry_name)?;
        Ok(entry.sign_data(data, hd_path)?)
    }
}

#[cfg(not(feature = "additional-libs"))]
mod secure_store_impl {
    use super::{Error, Print, PublicKey, Secret, SeedPhrase};

    pub fn get_public_key(_entry_name: &str, _index: Option<u32>) -> Result<PublicKey, Error> {
        Err(Error::FeatureNotEnabled)
    }

    pub fn delete_secret(_print: &Print, _entry_name: &str) -> Result<(), Error> {
        Err(Error::FeatureNotEnabled)
    }

    pub fn save_secret(
        _print: &Print,
        _name: &str,
        _seed_phrase: &SeedPhrase,
        _hd_path: Option<u32>,
        _overwrite: bool,
    ) -> Result<Secret, Error> {
        Err(Error::FeatureNotEnabled)
    }

    pub fn sign_tx_data(
        _entry_name: &str,
        _hd_path: Option<u32>,
        _data: &[u8],
    ) -> Result<Vec<u8>, Error> {
        Err(Error::FeatureNotEnabled)
    }
}
