use crate::{
    signer::{keyring::StellarEntry, secure_store},
    xdr::{self, DecoratedSignature, Signature, SignatureHint}
};

use ed25519_dalek::Signature as Ed25519Signature;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    SecureStore(#[from] secure_store::Error),
    #[error(transparent)]
    TryFromSlice(#[from] std::array::TryFromSliceError),
    #[error(transparent)]
    Xdr(#[from] xdr::Error),
}

pub struct SecureStoreEntry {
    pub name: String, //remove this
    pub hd_path: Option<usize>,
    pub entry: StellarEntry,
}

// still need the indirection of the secure_store mod so that we can handle things without the keyring crate
impl SecureStoreEntry {
    pub fn new(name: String, hd_path: Option<usize>) -> Self {
        SecureStoreEntry {
                name: name.clone(),
                hd_path,
                entry: StellarEntry::new(&name).unwrap() //fixme!
        }
    }

    pub fn get_public_key(&self) -> Result<stellar_strkey::ed25519::PublicKey, Error> {
        Ok(secure_store::get_public_key_with_entry(&self.entry, self.hd_path)?)
    }

    pub fn sign_tx_hash(&self, tx_hash: [u8; 32]) -> Result<DecoratedSignature, Error> {
        let hint = SignatureHint(
            secure_store::get_public_key_with_entry(&self.entry, self.hd_path)?.0[28..].try_into()?,
        );

        let signed_tx_hash = secure_store::sign_tx_data_with_entry(&self.entry, self.hd_path, &tx_hash)?;

        let signature = Signature(signed_tx_hash.clone().try_into()?);
        Ok(DecoratedSignature { hint, signature })
    }

    pub fn sign_payload(&self, payload: [u8; 32]) -> Result<Ed25519Signature, Error> {
        let signed_bytes = secure_store::sign_tx_data(&self.name, self.hd_path, &payload)?;
        let sig = Ed25519Signature::from_bytes(signed_bytes.as_slice().try_into()?);
        Ok(sig)
    }
}
