use base64::{engine::general_purpose::STANDARD as base64, Engine as _};
use ed25519_dalek::Signer;
use keyring::Entry;
use zeroize::Zeroize;

pub(crate) const KEYCHAIN_ENTRY_PREFIX: &str = "keychain:"; //TODO: does this belong here, or in secret?
pub(crate) const KEYCHAIN_ENTRY_SERVICE: &str = "org.stellar.cli";

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Keyring(#[from] keyring::Error),
    #[error(transparent)]
    Base64(#[from] base64::DecodeError),
}

pub struct StellarEntry {
    keyring: Entry,
}

impl StellarEntry {
    pub fn new(name: &str) -> Result<Self, Error> {
        Ok(StellarEntry {
            keyring: Entry::new(name, &whoami::username())?,
        })
    }

    pub fn set_password(&self, password: &[u8]) -> Result<(), Error> {
        let data = base64.encode(password);
        self.keyring.set_password(&data)?;
        Ok(())
    }

    pub fn get_password(&self) -> Result<Vec<u8>, Error> {
        Ok(base64.decode(self.keyring.get_password()?)?)
    }

    fn use_key<T>(
        &self,
        f: impl FnOnce(ed25519_dalek::SigningKey) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let mut key_vec = self.get_password()?;
        let mut key_bytes: [u8; 32] = key_vec.as_slice().try_into().unwrap();

        let result = {
            // Use this scope to ensure the keypair is zeroized
            let keypair = ed25519_dalek::SigningKey::from_bytes(&key_bytes);
            f(keypair)?
        };
        key_vec.zeroize();
        key_bytes.zeroize();
        Ok(result)
    }

    pub fn get_public_key(&self) -> Result<stellar_strkey::ed25519::PublicKey, Error> {
        self.use_key(|keypair| {
            Ok(stellar_strkey::ed25519::PublicKey(
                *keypair.verifying_key().as_bytes(),
            ))
        })
    }

    pub fn sign_data(&self, data: &[u8]) -> Result<Vec<u8>, Error> {
        self.use_key(|keypair| {
            let signature = keypair.sign(data);
            Ok(signature.to_bytes().to_vec())
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use keyring::{
        mock,
        set_default_credential_builder,
    };

    #[test]
    fn test_sign_data() {
        set_default_credential_builder(mock::default_credential_builder());

        let secret = crate::config::secret::Secret::from_seed(None).unwrap();
        let pub_key = secret.public_key(None).unwrap();
        let key_pair = secret.key_pair(None).unwrap();

        let entry = StellarEntry::new("test").unwrap();

        // set the password
        let set_password_result = entry.set_password(&key_pair.to_bytes());
        assert!(set_password_result.is_ok());

        // confirm that we can get the public key from the entry and that it matches the one we set
        let get_public_key_result = entry.get_public_key();
        assert!(get_public_key_result.is_ok());
        assert_eq!(pub_key, get_public_key_result.unwrap());
    }
}
