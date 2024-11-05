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

    pub fn get_public_key(&self) -> Result<stellar_strkey::ed25519::PublicKey, Error> {
        let mut key_vec = self.get_password()?;
        let mut key_bytes: [u8; 32] = key_vec.as_slice().try_into().unwrap();

        let pub_key = {
            // Use this scope to ensure the keypair is zeroized
            let keypair = ed25519_dalek::SigningKey::from_bytes(&key_bytes);
            stellar_strkey::ed25519::PublicKey(*keypair.verifying_key().as_bytes())
        };
        key_vec.zeroize();
        key_bytes.zeroize();
        Ok(pub_key)
    }
}

pub fn sign_data(name: &str, data: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    // Retrieve the key from the secure storage
    let entry = Entry::new("stellar", name)?;
    let key_bytes: [u8; 32] = entry.get_secret()?.try_into().unwrap();
    // Create a keypair from the retrieved bytes
    let keypair = ed25519_dalek::SigningKey::from_bytes(&key_bytes);

    // Sign the data
    let signature = keypair.sign(data);

    // Clear the key from memory
    let mut key_bytes = key_bytes;
    key_bytes.zeroize();

    Ok(signature.to_bytes().to_vec())
}

pub fn add_key(name: &str, key_bytes: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    // Create a new keyring entry for "stellar"
    StellarEntry::new(name)?.set_password(key_bytes)?;
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_sign_data() -> Result<(), Box<dyn std::error::Error>> {
        let secret = crate::config::secret::Secret::from_seed(None)?;
        let pub_key = secret.public_key(None)?;
        let key_pair = secret.key_pair(None)?;

        add_key("test", &key_pair.to_bytes()).unwrap();
        let pub_key_2 = get_public_key("test")?;
        assert_eq!(pub_key, pub_key_2);
        Ok(())
    }
}
