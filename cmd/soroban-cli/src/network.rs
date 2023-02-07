use sha2::{Digest, Sha256};

pub static SANDBOX_NETWORK_PASSPHRASE: &str = "Local Sandbox Stellar Network ; September 2022";

#[must_use]
pub fn sandbox_network_id() -> [u8; 32] {
    Sha256::digest(SANDBOX_NETWORK_PASSPHRASE.as_bytes()).into()
}
