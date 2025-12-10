use stellar_strkey::ed25519::PublicKey;

use crate::{
    print::Print,
    xdr::{self, DecoratedSignature},
};

#[cfg(feature = "additional-libs")]
use crate::{
    signer::keyring::{self, StellarEntry},
    xdr::{Signature, SignatureHint},
};
#[cfg(feature = "additional-libs")]
use std::sync::Arc;

use ed25519_dalek::Signature as Ed25519Signature;

use sep5::SeedPhrase;

#[cfg(feature = "additional-libs")]
const ENTRY_SERVICE: &str = "org.stellar.cli";
pub(crate) const ENTRY_PREFIX: &str = "secure_store:";

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[cfg(feature = "additional-libs")]
    #[error(transparent)]
    Keyring(#[from] keyring::Error),

    #[error(transparent)]
    TryFromSlice(#[from] std::array::TryFromSliceError),

    #[error(transparent)]
    Xdr(#[from] xdr::Error),

    #[error("Secure Store keys are not allowed: additional-libs feature must be enabled")]
    FeatureNotEnabled,
}

#[derive(Debug, Clone)]
pub struct SecureStoreEntry {
    pub hd_path: Option<usize>,
    #[cfg(feature = "additional-libs")]
    pub entry: Arc<StellarEntry>,
}

#[cfg(feature = "additional-libs")]
impl SecureStoreEntry {
    pub fn new(name: String, hd_path: Option<usize>) -> Result<Self, Error> {
        Ok(Self {
            hd_path,
            entry: Arc::new(StellarEntry::new(&name)?),
        })
    }

    pub fn get_public_key(&self) -> Result<PublicKey, Error> {
        Ok(self.entry.get_public_key(self.hd_path)?)
    }

    pub fn delete_secret(&self, print: &Print) -> Result<(), Error> {
        Ok(self.entry.delete_seed_phrase(print)?)
    }

    pub fn create_and_save(
        entry_name: &str,
        seed_phrase: &SeedPhrase,
        print: &Print,
    ) -> Result<String, Error> {
        let entry_name_with_prefix = format!("{ENTRY_PREFIX}{ENTRY_SERVICE}-{entry_name}");

        let s = Self::new(entry_name_with_prefix.clone(), None)?;
        s.entry.write(seed_phrase.clone(), print)?;

        Ok(entry_name_with_prefix)
    }

    pub fn sign_tx_data(&self, data: &[u8]) -> Result<Vec<u8>, Error> {
        Ok(self.entry.sign_data(data, self.hd_path)?)
    }

    pub fn sign_tx_hash(&self, tx_hash: [u8; 32]) -> Result<DecoratedSignature, Error> {
        let hint = SignatureHint(self.get_public_key()?.0[28..].try_into()?);

        let signed_tx_hash = self.sign_tx_data(&tx_hash)?;

        let signature = Signature(signed_tx_hash.clone().try_into()?);
        Ok(DecoratedSignature { hint, signature })
    }

    pub fn sign_payload(&self, payload: [u8; 32]) -> Result<Ed25519Signature, Error> {
        let signed_bytes = self.sign_tx_data(&payload)?;

        let sig = Ed25519Signature::from_bytes(signed_bytes.as_slice().try_into()?);
        Ok(sig)
    }
}

#[cfg(not(feature = "additional-libs"))]
impl SecureStoreEntry {
    pub fn new(_name: String, _hd_path: Option<usize>) -> Result<Self, Error> {
        Err(Error::FeatureNotEnabled)
    }

    pub fn get_public_key(&self) -> Result<PublicKey, Error> {
        Err(Error::FeatureNotEnabled)
    }

    pub fn delete_secret(&self, _print: &Print) -> Result<(), Error> {
        Err(Error::FeatureNotEnabled)
    }

    pub fn create_and_save(
        _entry_name: &str,
        _seed_phrase: &SeedPhrase,
        _print: &Print,
    ) -> Result<String, Error> {
        Err(Error::FeatureNotEnabled)
    }

    pub fn sign_tx_data(_data: &[u8]) -> Result<Vec<u8>, Error> {
        Err(Error::FeatureNotEnabled)
    }

    pub fn sign_tx_hash(&self, _tx_hash: [u8; 32]) -> Result<DecoratedSignature, Error> {
        Err(Error::FeatureNotEnabled)
    }

    pub fn sign_payload(&self, _payload: [u8; 32]) -> Result<Ed25519Signature, Error> {
        Err(Error::FeatureNotEnabled)
    }
}
