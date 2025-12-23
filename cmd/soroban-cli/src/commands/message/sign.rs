use std::io::{self, Read};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use clap::Parser;
use ed25519_dalek::Signer as _;
use sha2::{Digest, Sha256};

use crate::{
    commands::global,
    config::{locator, secret},
    signer::{self, SecureStoreEntry},
};

use super::SEP53_PREFIX;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Locator(#[from] locator::Error),

    #[error(transparent)]
    Secret(#[from] secret::Error),

    #[error(transparent)]
    Signer(#[from] signer::Error),

    #[error(transparent)]
    Io(#[from] io::Error),

    #[error(transparent)]
    Base64(#[from] base64::DecodeError),

    #[error(transparent)]
    StrKey(#[from] stellar_strkey::DecodeError),

    #[error(transparent)]
    Ed25519(#[from] ed25519_dalek::SignatureError),

    #[error("No signing key provided. Use --sign-with-key")]
    NoSigningKey,

    #[error("Ledger signing of arbitrary messages is not yet supported")]
    LedgerNotSupported,
}

#[derive(Debug, Parser, Clone)]
#[group(skip)]
pub struct Cmd {
    /// The message to sign. If not provided, reads from stdin.
    #[arg()]
    pub message: Option<String>,

    /// Treat the message as base64-encoded binary data
    #[arg(long)]
    pub base64: bool,

    /// Sign with a local key. Can be an identity (--sign-with-key alice),
    /// a secret key (--sign-with-key SC36...), or a seed phrase
    /// (--sign-with-key "kite urban...").
    #[arg(long, env = "STELLAR_SIGN_WITH_KEY")]
    pub sign_with_key: Option<String>,

    /// If using a seed phrase to sign, sets which hierarchical deterministic
    /// path to use, e.g. `m/44'/148'/{hd_path}`. Example: `--hd-path 1`. Default: `0`
    #[arg(long)]
    pub hd_path: Option<usize>,

    /// Sign with a Ledger hardware wallet
    #[arg(long, conflicts_with = "sign_with_key", env = "STELLAR_SIGN_WITH_LEDGER")]
    pub sign_with_ledger: bool,

    #[command(flatten)]
    pub locator: locator::Args,
}

/// Output format for signed messages
#[derive(serde::Serialize)]
struct SignedMessageOutput {
    /// The public key (address) that signed the message
    signer: String,
    /// The original message (as provided or base64 if binary)
    message: String,
    /// The base64-encoded signature
    signature: String,
}

impl Cmd {
    #[allow(clippy::unused_async)]
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        // Get the message bytes
        let message_bytes = self.get_message_bytes()?;

        // Create the SEP-53 payload: prefix + message
        let mut payload = Vec::with_capacity(SEP53_PREFIX.len() + message_bytes.len());
        payload.extend_from_slice(SEP53_PREFIX.as_bytes());
        payload.extend_from_slice(&message_bytes);

        // Hash the payload with SHA-256
        let hash: [u8; 32] = Sha256::digest(&payload).into();

        // Get the signer and sign
        let (public_key, signature) = self.sign_hash(hash)?;

        // Encode signature as base64
        let signature_base64 = BASE64.encode(signature.to_bytes());

        // Output the result
        let output = SignedMessageOutput {
            signer: public_key.to_string(),
            message: if self.base64 {
                BASE64.encode(&message_bytes)
            } else {
                String::from_utf8_lossy(&message_bytes).to_string()
            },
            signature: signature_base64.clone(),
        };

        if global_args.quiet {
            // In quiet mode, just output the signature
            println!("{signature_base64}");
        } else {
            // Output as formatted text
            println!("Signer: {}", output.signer);
            println!("Signature: {}", output.signature);
        }

        Ok(())
    }

    fn sign_hash(
        &self,
        hash: [u8; 32],
    ) -> Result<(stellar_strkey::ed25519::PublicKey, ed25519_dalek::Signature), Error> {
        if self.sign_with_ledger {
            // Ledger doesn't support signing arbitrary messages yet
            return Err(Error::LedgerNotSupported);
        }

        let key_or_name = self.sign_with_key.as_deref().ok_or(Error::NoSigningKey)?;
        let secret = self.locator.get_secret_key(key_or_name)?;

        match &secret {
            secret::Secret::SecretKey { .. } | secret::Secret::SeedPhrase { .. } => {
                let signing_key = secret.key_pair(self.hd_path)?;
                let public_key = stellar_strkey::ed25519::PublicKey::from_payload(
                    signing_key.verifying_key().as_bytes(),
                )?;
                let signature = signing_key.sign(&hash);
                Ok((public_key, signature))
            }
            secret::Secret::Ledger => {
                // Ledger doesn't support signing arbitrary messages yet
                Err(Error::LedgerNotSupported)
            }
            secret::Secret::SecureStore { entry_name } => {
                let entry = SecureStoreEntry {
                    name: entry_name.clone(),
                    hd_path: self.hd_path,
                };
                let public_key = entry.get_public_key()?;
                let signature = entry.sign_payload(hash)?;
                Ok((public_key, signature))
            }
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::secret::Secret;
    use std::str::FromStr;

    // Use a known valid test key from the codebase
    const TEST_SECRET_KEY: &str = "SBF5HLRREHMS36XZNTUSKZ6FTXDZGNXOHF4EXKUL5UCWZLPBX3NGJ4BH";
    const TEST_PUBLIC_KEY: &str = "GAREAZZQWHOCBJS236KIE3AWYBVFLSBK7E5UW3ICI3TCRWQKT5LNLCEZ";

    fn get_test_signing_key() -> ed25519_dalek::SigningKey {
        let secret = Secret::from_str(TEST_SECRET_KEY).unwrap();
        secret.key_pair(None).unwrap()
    }

    fn sign_message(message_bytes: &[u8], signing_key: &ed25519_dalek::SigningKey) -> String {
        // Create SEP-53 payload
        let mut payload = Vec::with_capacity(SEP53_PREFIX.len() + message_bytes.len());
        payload.extend_from_slice(SEP53_PREFIX.as_bytes());
        payload.extend_from_slice(message_bytes);

        // Hash with SHA-256
        let hash: [u8; 32] = Sha256::digest(&payload).into();

        // Sign
        let signature = signing_key.sign(&hash);

        // Return base64-encoded signature
        BASE64.encode(signature.to_bytes())
    }

    fn verify_signature(
        message_bytes: &[u8],
        signature_base64: &str,
        signing_key: &ed25519_dalek::SigningKey,
    ) -> bool {
        use ed25519_dalek::Verifier;

        // Create SEP-53 payload
        let mut payload = Vec::with_capacity(SEP53_PREFIX.len() + message_bytes.len());
        payload.extend_from_slice(SEP53_PREFIX.as_bytes());
        payload.extend_from_slice(message_bytes);

        // Hash with SHA-256
        let hash: [u8; 32] = Sha256::digest(&payload).into();

        // Decode signature
        let signature_bytes = BASE64.decode(signature_base64).unwrap();
        let signature = ed25519_dalek::Signature::from_slice(&signature_bytes).unwrap();

        // Verify
        signing_key.verifying_key().verify(&hash, &signature).is_ok()
    }

    #[test]
    fn test_sign_and_verify_ascii_message() {
        let signing_key = get_test_signing_key();

        // Verify public key matches expected
        let public_key = stellar_strkey::ed25519::PublicKey::from_payload(
            signing_key.verifying_key().as_bytes(),
        )
        .unwrap();
        assert_eq!(public_key.to_string(), TEST_PUBLIC_KEY);

        // Sign and verify
        let message = "Hello, World!";
        let signature = sign_message(message.as_bytes(), &signing_key);
        assert!(verify_signature(message.as_bytes(), &signature, &signing_key));
    }

    #[test]
    fn test_sign_and_verify_utf8_message() {
        let signing_key = get_test_signing_key();

        // Sign and verify Japanese text
        let message = "こんにちは、世界！";
        let signature = sign_message(message.as_bytes(), &signing_key);
        assert!(verify_signature(message.as_bytes(), &signature, &signing_key));
    }

    #[test]
    fn test_sign_and_verify_binary_message() {
        let signing_key = get_test_signing_key();

        // Sign and verify binary data
        let message_base64 = "2zZDP1sa1BVBfLP7TeeMk3sUbaxAkUhBhDiNdrksaFo=";
        let message_bytes = BASE64.decode(message_base64).unwrap();
        let signature = sign_message(&message_bytes, &signing_key);
        assert!(verify_signature(&message_bytes, &signature, &signing_key));
    }

    #[test]
    fn test_sep53_prefix_is_correct() {
        // Verify the SEP-53 prefix is as specified
        assert_eq!(SEP53_PREFIX, "Stellar Signed Message:\n");
    }

    #[test]
    fn test_wrong_signature_fails_verification() {
        let signing_key = get_test_signing_key();

        let message1 = "Hello, World!";
        let message2 = "Goodbye, World!";

        // Sign message1
        let signature = sign_message(message1.as_bytes(), &signing_key);

        // Verify fails with different message
        assert!(!verify_signature(message2.as_bytes(), &signature, &signing_key));
    }
}
