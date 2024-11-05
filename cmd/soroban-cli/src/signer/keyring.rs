use base64::{engine::general_purpose::STANDARD as base64, Engine as _};
use ed25519_dalek::Signer;
use keyring::Entry;
use zeroize::Zeroize;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Keyring(#[from] keyring::Error),
    #[error(transparent)]
    Base64(#[from] base64::DecodeError),
}

pub struct StellarEntry {
    name: String,
}

impl TryFrom<&StellarEntry> for Entry {
    type Error = Error;
    fn try_from(StellarEntry { name }: &StellarEntry) -> Result<Self, Self::Error> {
        Ok(Entry::new(
            &format!("org.stellar.cli.{name}"),
            &whoami::username(),
        )?)
    }
}

impl StellarEntry {
    pub fn new(name: &str) -> Result<Self, Error> {
        Ok(StellarEntry {
            name: name.to_string(),
        })
    }

    pub fn set_password(&self, password: &[u8]) -> Result<(), Error> {
        let data = base64.encode(password);
        let entry: Entry = self.try_into()?;
        entry.set_password(&data)?;
        Ok(())
    }

    pub fn get_password(&self) -> Result<Vec<u8>, Error> {
        let entry: Entry = self.try_into()?;
        Ok(base64.decode(entry.get_password()?)?)
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

    #[test]
    fn test_sign_data() -> Result<(), Box<dyn std::error::Error>> {
        let secret = crate::config::secret::Secret::from_seed(None)?;
        let pub_key = secret.public_key(None)?;
        let key_pair = secret.key_pair(None)?;
        let entry = StellarEntry::new("test")?;
        entry.set_password(&key_pair.to_bytes());
        let pub_key_2 = entry.get_public_key()?;
        assert_eq!(pub_key, pub_key_2);
        Ok(())
    }
}
