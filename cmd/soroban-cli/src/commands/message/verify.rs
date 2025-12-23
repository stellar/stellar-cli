use std::io::{self, Read};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use clap::Parser;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use sha2::{Digest, Sha256};

use crate::config::{locator, secret};

use super::SEP53_PREFIX;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Locator(#[from] locator::Error),

    #[error(transparent)]
    Secret(#[from] secret::Error),

    #[error(transparent)]
    Io(#[from] io::Error),

    #[error(transparent)]
    Base64(#[from] base64::DecodeError),

    #[error(transparent)]
    StrKey(#[from] stellar_strkey::DecodeError),

    #[error(transparent)]
    Ed25519(#[from] ed25519_dalek::SignatureError),

    #[error("Signature verification failed")]
    VerificationFailed,

    #[error("Invalid signature length: expected 64 bytes, got {0}")]
    InvalidSignatureLength(usize),
}

#[derive(Debug, Parser, Clone)]
#[group(skip)]
pub struct Cmd {
    /// The message to verify. If not provided, reads from stdin.
    #[arg()]
    pub message: Option<String>,

    /// The base64-encoded signature to verify
    #[arg(long, short = 's')]
    pub signature: String,

    /// The public key to verify against.
    /// Can be a Stellar public key (G...) or an identity name.
    #[arg(long, short = 'p')]
    pub public_key: String,

    /// Treat the message as base64-encoded binary data
    #[arg(long)]
    pub base64: bool,

    #[command(flatten)]
    pub locator: locator::Args,
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        // Get the message bytes
        let message_bytes = self.get_message_bytes()?;

        // Create the SEP-53 payload: prefix + message
        let mut payload = Vec::with_capacity(SEP53_PREFIX.len() + message_bytes.len());
        payload.extend_from_slice(SEP53_PREFIX.as_bytes());
        payload.extend_from_slice(&message_bytes);

        // Hash the payload with SHA-256
        let hash: [u8; 32] = Sha256::digest(&payload).into();

        // Decode the signature
        let signature_bytes = BASE64.decode(&self.signature)?;
        if signature_bytes.len() != 64 {
            return Err(Error::InvalidSignatureLength(signature_bytes.len()));
        }
        let signature = Signature::from_slice(&signature_bytes)?;

        // Get the public key
        let public_key_bytes = self.get_public_key_bytes()?;
        let verifying_key = VerifyingKey::from_bytes(&public_key_bytes)?;

        // Verify the signature
        if verifying_key.verify(&hash, &signature).is_ok() {
            let public_key = stellar_strkey::ed25519::PublicKey(public_key_bytes);
            println!("Signature valid");
            println!("Signer: {public_key}");
            Ok(())
        } else {
            eprintln!("Signature invalid");
            Err(Error::VerificationFailed)
        }
    }

    fn get_message_bytes(&self) -> Result<Vec<u8>, Error> {
        let message_str = if let Some(msg) = &self.message {
            msg.clone()
        } else {
            // Read from stdin
            let mut buffer = String::new();
            io::stdin().read_to_string(&mut buffer)?;
            // Remove trailing newline if present
            if buffer.ends_with('\n') {
                buffer.pop();
                if buffer.ends_with('\r') {
                    buffer.pop();
                }
            }
            buffer
        };

        if self.base64 {
            // Decode base64 input
            Ok(BASE64.decode(&message_str)?)
        } else {
            // Use UTF-8 encoded message
            Ok(message_str.into_bytes())
        }
    }

    fn get_public_key_bytes(&self) -> Result<[u8; 32], Error> {
        // First, try to parse as a Stellar public key directly
        if let Ok(pk) = stellar_strkey::ed25519::PublicKey::from_string(&self.public_key) {
            return Ok(pk.0);
        }

        // Otherwise, try to look it up as an identity
        let secret = self.locator.get_secret_key(&self.public_key)?;
        let pk = secret.public_key(None)?;
        Ok(pk.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test vectors from SEP-53
    const TEST_PUBLIC_KEY: &str = "GBXFXNDLV4LSWA4VB7YIL5GBD7BVNR22SGBTDKMO2SBZZHDXSKZYCP7L";

    // Test case 1: ASCII message
    const TEST_MESSAGE_1: &str = "Hello, World!";
    const TEST_SIGNATURE_1: &str =
        "fO5dbYhXUhBMhe6kId/cuVq/AfEnHRHEvsP8vXh03M1uLpi5e46yO2Q8rEBzu3feXQewcQE5GArp88u6ePK6BA==";

    // Test case 2: Japanese text (UTF-8)
    const TEST_MESSAGE_2: &str = "こんにちは、世界！";
    const TEST_SIGNATURE_2: &str =
        "CDU265Xs8y3OWbB/56H9jPgUss5G9A0qFuTqH2zs2YDgTm+++dIfmAEceFqB7bhfN3am59lCtDXrCtwH2k1GBA==";

    // Test case 3: Binary data (base64 encoded in test vector)
    const TEST_MESSAGE_3_BASE64: &str = "2zZDP1sa1BVBfLP7TeeMk3sUbaxAkUhBhDiNdrksaFo=";
    const TEST_SIGNATURE_3: &str =
        "VA1+7hefNwv2NKScH6n+Sljj15kLAge+M2wE7fzFOf+L0MMbssA1mwfJZRyyrhBORQRle10X1Dxpx+UOI4EbDQ==";

    fn verify_message(
        message_bytes: &[u8],
        signature_base64: &str,
        public_key_str: &str,
    ) -> bool {
        // Create SEP-53 payload
        let mut payload = Vec::with_capacity(SEP53_PREFIX.len() + message_bytes.len());
        payload.extend_from_slice(SEP53_PREFIX.as_bytes());
        payload.extend_from_slice(message_bytes);

        // Hash with SHA-256
        let hash: [u8; 32] = Sha256::digest(&payload).into();

        // Decode signature
        let signature_bytes = BASE64.decode(signature_base64).unwrap();
        let signature = Signature::from_slice(&signature_bytes).unwrap();

        // Decode public key
        let public_key = stellar_strkey::ed25519::PublicKey::from_string(public_key_str).unwrap();
        let verifying_key = VerifyingKey::from_bytes(&public_key.0).unwrap();

        // Verify
        verifying_key.verify(&hash, &signature).is_ok()
    }

    #[test]
    fn test_verify_ascii_message() {
        assert!(verify_message(
            TEST_MESSAGE_1.as_bytes(),
            TEST_SIGNATURE_1,
            TEST_PUBLIC_KEY
        ));
    }

    #[test]
    fn test_verify_utf8_message() {
        assert!(verify_message(
            TEST_MESSAGE_2.as_bytes(),
            TEST_SIGNATURE_2,
            TEST_PUBLIC_KEY
        ));
    }

    #[test]
    fn test_verify_binary_message() {
        let message_bytes = BASE64.decode(TEST_MESSAGE_3_BASE64).unwrap();
        assert!(verify_message(&message_bytes, TEST_SIGNATURE_3, TEST_PUBLIC_KEY));
    }

    #[test]
    fn test_verify_wrong_signature() {
        // Use signature from message 2 with message 1
        assert!(!verify_message(
            TEST_MESSAGE_1.as_bytes(),
            TEST_SIGNATURE_2,
            TEST_PUBLIC_KEY
        ));
    }

    #[test]
    fn test_verify_wrong_public_key() {
        // Use a different public key
        let wrong_key = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF";
        assert!(!verify_message(
            TEST_MESSAGE_1.as_bytes(),
            TEST_SIGNATURE_1,
            wrong_key
        ));
    }
}
