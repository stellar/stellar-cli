use std::io::{self, Read};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use clap::Parser;
use sha2::{Digest, Sha256};

use crate::{
    commands::global,
    config::{locator, secret},
    print::Print,
    signer::{self, Signer},
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
    /// The message to sign. If not provided, reads from stdin. This should **not** include
    /// the SEP-53 prefix "Stellar Signed Message:\n", as it will be added automatically.
    #[arg()]
    pub message: Option<String>,

    /// Treat the message as base64-encoded binary data
    #[arg(long)]
    pub base64: bool,

    // @dev: Ledger and Lab don't support signing arbitrary messages yet. Once they do, use `sign_with::Args` here.
    /// Sign with a local key or key saved in OS secure storage. Can be an identity (--sign-with-key alice), a secret key (--sign-with-key SC36…), or a seed phrase (--sign-with-key "kite urban…"). If using seed phrase, `--hd-path` defaults to the `0` path.
    #[arg(long, env = "STELLAR_SIGN_WITH_KEY")]
    pub sign_with_key: String,

    #[arg(long)]
    /// If using a seed phrase to sign, sets which hierarchical deterministic path to use, e.g. `m/44'/148'/{hd_path}`. Example: `--hd-path 1`. Default: `0`
    pub hd_path: Option<usize>,

    #[command(flatten)]
    pub locator: locator::Args,
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let print = Print::new(global_args.quiet);

        // Get the message bytes
        let message_bytes = self.get_message_bytes()?;

        // Get the signer
        let key_or_name = &self.sign_with_key;
        let secret = self.locator.get_secret_key(key_or_name)?;
        let signer = secret.signer(self.hd_path, print.clone()).await?;
        let public_key = signer.get_public_key()?;

        // Encode signature as base64
        let signature_base64 = sep_53_sign(&message_bytes, signer)?;

        print.infoln(format!("Signer: {public_key}"));
        let message_display = if self.base64 {
            BASE64.encode(&message_bytes)
        } else {
            String::from_utf8_lossy(&message_bytes).to_string()
        };
        print.infoln(format!("Message: {message_display}"));
        print.println_stdout(signature_base64);
        Ok(())
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

/// Sign the given message bytes with the provided signer, returning the base64-encoded signature.
///
/// Expects the message bytes to be the raw message (without SEP-53 prefix).
fn sep_53_sign(message_bytes: &[u8], signer: Signer) -> Result<String, Error> {
    // Create SEP-53 payload
    let mut payload = Vec::with_capacity(SEP53_PREFIX.len() + message_bytes.len());
    payload.extend_from_slice(SEP53_PREFIX.as_bytes());
    payload.extend_from_slice(message_bytes);
    let hash: [u8; 32] = Sha256::digest(&payload).into();

    let signature = signer.sign_payload(hash)?;

    Ok(BASE64.encode(signature.to_bytes()))
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;
    use crate::{config::secret::Secret, utils::into_signing_key};

    // Public key = GBXFXNDLV4LSWA4VB7YIL5GBD7BVNR22SGBTDKMO2SBZZHDXSKZYCP7L
    const TEST_SECRET_KEY: &str = "SAKICEVQLYWGSOJS4WW7HZJWAHZVEEBS527LHK5V4MLJALYKICQCJXMW";

    fn setup_locator() -> locator::Args {
        let temp_dir = tempfile::tempdir().unwrap();
        locator::Args {
            global: false,
            config_dir: Some(temp_dir.path().to_path_buf()),
        }
    }

    fn build_signer_for_test_key() -> Signer {
        let secret = Secret::from_str(TEST_SECRET_KEY).unwrap();
        let private_key = secret.private_key(None).unwrap();
        let signing_key = into_signing_key(&private_key);
        Signer {
            kind: signer::SignerKind::Local(signer::LocalKey { key: signing_key }),
            print: Print::new(true),
        }
    }

    #[test]
    fn test_sign_simple() {
        // SEP-53 - test case 1
        let message = "Hello, World!".to_string();
        let expected_signature = "fO5dbYhXUhBMhe6kId/cuVq/AfEnHRHEvsP8vXh03M1uLpi5e46yO2Q8rEBzu3feXQewcQE5GArp88u6ePK6BA==";

        let locator = setup_locator();
        let cmd = super::Cmd {
            message: Some(message),
            base64: false,
            sign_with_key: TEST_SECRET_KEY.to_string(),
            hd_path: None,
            locator: locator.clone(),
        };
        let signer = build_signer_for_test_key();

        let message_bytes = cmd.get_message_bytes().unwrap();
        let signature_base64 = sep_53_sign(&message_bytes, signer).unwrap();

        assert_eq!(signature_base64, expected_signature);
    }

    #[test]
    fn test_sign_japanese() {
        // SEP-53 - test case 2
        let message = "こんにちは、世界！".to_string();
        let expected_signature = "CDU265Xs8y3OWbB/56H9jPgUss5G9A0qFuTqH2zs2YDgTm+++dIfmAEceFqB7bhfN3am59lCtDXrCtwH2k1GBA==";

        let locator = setup_locator();
        let cmd = super::Cmd {
            message: Some(message),
            base64: false,
            sign_with_key: TEST_SECRET_KEY.to_string(),
            hd_path: None,
            locator: locator.clone(),
        };
        let signer = build_signer_for_test_key();

        let message_bytes = cmd.get_message_bytes().unwrap();
        let signature_base64 = sep_53_sign(&message_bytes, signer).unwrap();

        assert_eq!(signature_base64, expected_signature);
    }

    #[test]
    fn test_sign_base64() {
        // SEP-53 - test case 3
        let message = "2zZDP1sa1BVBfLP7TeeMk3sUbaxAkUhBhDiNdrksaFo=".to_string();
        let expected_signature = "VA1+7hefNwv2NKScH6n+Sljj15kLAge+M2wE7fzFOf+L0MMbssA1mwfJZRyyrhBORQRle10X1Dxpx+UOI4EbDQ==";

        let locator = setup_locator();
        let cmd = super::Cmd {
            message: Some(message),
            base64: true,
            sign_with_key: TEST_SECRET_KEY.to_string(),
            hd_path: None,
            locator: locator.clone(),
        };
        let signer = build_signer_for_test_key();

        let message_bytes = cmd.get_message_bytes().unwrap();
        let signature_base64 = sep_53_sign(&message_bytes, signer).unwrap();

        assert_eq!(signature_base64, expected_signature);
    }
}
