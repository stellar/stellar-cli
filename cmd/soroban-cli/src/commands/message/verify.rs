use std::io::{self, Read};

use crate::{
    commands::global,
    config::{key, locator, secret},
    print::Print,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use clap::Parser;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use sha2::{Digest, Sha256};

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
    Hex(#[from] hex::FromHexError),

    #[error(transparent)]
    StrKey(#[from] stellar_strkey::DecodeError),

    #[error(transparent)]
    Key(#[from] key::Error),

    #[error(transparent)]
    Ed25519(#[from] ed25519_dalek::SignatureError),

    #[error(transparent)]
    Address(#[from] crate::config::address::Error),

    #[error("Signature verification failed")]
    VerificationFailed,

    #[error("Invalid signature length: expected 64 bytes, got {0}")]
    InvalidSignatureLength(usize),
}

#[derive(Debug, Parser, Clone)]
#[group(skip)]
pub struct Cmd {
    /// The message to verify. If not provided, reads from stdin. This should **not** include
    /// the SEP-53 prefix "Stellar Signed Message:\n", as it will be added automatically.
    #[arg()]
    pub message: Option<String>,

    /// Treat the message as base64-encoded binary data
    #[arg(long)]
    pub base64: bool,

    /// Verify a raw signature: the message bytes were signed directly, without
    /// the SEP-53 prefix or SHA-256 hashing, and `--signature` is hex-encoded.
    #[arg(long)]
    pub raw: bool,

    /// The signature to verify. Base64-encoded by default, or hex-encoded with `--raw`.
    #[arg(long, short = 's')]
    pub signature: String,

    /// The public key to verify the signature against. Can be an identity (--public-key alice),
    /// a public key (--public-key GDKW...).
    #[arg(long, short = 'p')]
    pub public_key: String,

    /// If public key identity is a seed phrase use this hd path, default is 0
    #[arg(long)]
    pub hd_path: Option<u32>,

    #[command(flatten)]
    pub locator: locator::Args,
}

impl Cmd {
    pub fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let print = Print::new(global_args.quiet);

        let message_bytes = self.get_message_bytes()?;

        // Decode the signature: hex for the raw path, base64 for SEP-53.
        let signature_bytes = if self.raw {
            hex::decode(&self.signature)?
        } else {
            BASE64.decode(&self.signature)?
        };
        if signature_bytes.len() != 64 {
            return Err(Error::InvalidSignatureLength(signature_bytes.len()));
        }
        let signature = Signature::from_slice(&signature_bytes)?;

        // Get the verifying key
        let public_key = self.get_public_key()?;
        print.infoln(format!("Verifying signature against: {public_key}"));
        let verifying_key = VerifyingKey::from_bytes(&public_key.0)?;

        // The raw path verifies over the exact message bytes; the default path
        // verifies over SHA-256(SEP-53 prefix + message).
        let verified = if self.raw {
            verifying_key.verify(&message_bytes, &signature).is_ok()
        } else {
            let mut payload = Vec::with_capacity(SEP53_PREFIX.len() + message_bytes.len());
            payload.extend_from_slice(SEP53_PREFIX.as_bytes());
            payload.extend_from_slice(&message_bytes);
            let hash: [u8; 32] = Sha256::digest(&payload).into();
            verifying_key.verify(&hash, &signature).is_ok()
        };

        if verified {
            print.checkln("Signature valid");
            Ok(())
        } else {
            print.errorln("Signature invalid");
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

    fn get_public_key(&self) -> Result<stellar_strkey::ed25519::PublicKey, Error> {
        // Try public-only parsing first (G... or M...); fall through to alias
        // resolution only when the input doesn't parse as any key.
        let key = match key::Key::parse_public_only(&self.public_key) {
            Ok(key) => key,
            Err(err @ key::Error::PublicKeyExpected) => return Err(Error::Key(err)),
            Err(_) => self.locator.read_key(&self.public_key)?,
        };
        let account = key
            .muxed_account(self.hd_path)
            .map_err(crate::config::address::Error::from)?;
        let bytes = match account {
            soroban_sdk::xdr::MuxedAccount::Ed25519(uint256) => uint256.0,
            soroban_sdk::xdr::MuxedAccount::MuxedEd25519(muxed_account) => muxed_account.ed25519.0,
        };
        Ok(stellar_strkey::ed25519::PublicKey(bytes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Public key = GBXFXNDLV4LSWA4VB7YIL5GBD7BVNR22SGBTDKMO2SBZZHDXSKZYCP7L
    const TEST_PUBLIC_KEY: &str = "GBXFXNDLV4LSWA4VB7YIL5GBD7BVNR22SGBTDKMO2SBZZHDXSKZYCP7L";
    const FALSE_PUBLIC_KEY: &str = "GAREAZZQWHOCBJS236KIE3AWYBVFLSBK7E5UW3ICI3TCRWQKT5LNLCEZ";
    const FALSE_SIGNATURE: &str =
        "+F//cUINZgTe4vZNXOEJTchDgEYlvy+iGFH3P65KeVhoyZgAsmGRRYAQLVqgY9J3PAlHPbSSeU5advhswmAfDg==";

    fn setup_locator() -> locator::Args {
        let temp_dir = tempfile::tempdir().unwrap();
        locator::Args {
            config_dir: Some(temp_dir.path().to_path_buf()),
        }
    }

    fn global_args() -> global::Args {
        global::Args {
            quiet: true,
            ..Default::default()
        }
    }

    #[test]
    fn test_verify_simple() {
        // SEP-53 - test case 1
        let message = "Hello, World!".to_string();
        let signature = "fO5dbYhXUhBMhe6kId/cuVq/AfEnHRHEvsP8vXh03M1uLpi5e46yO2Q8rEBzu3feXQewcQE5GArp88u6ePK6BA==";

        let global = global_args();
        let locator = setup_locator();
        let cmd = super::Cmd {
            message: Some(message),
            base64: false,
            raw: false,
            signature: signature.to_string(),
            public_key: TEST_PUBLIC_KEY.to_string(),
            hd_path: None,
            locator: locator.clone(),
        };
        let successful = cmd.run(&global);
        assert!(successful.is_ok());
    }

    #[test]
    fn test_verify_japanese() {
        // SEP-53 - test case 2
        let message = "こんにちは、世界！".to_string();
        let signature = "CDU265Xs8y3OWbB/56H9jPgUss5G9A0qFuTqH2zs2YDgTm+++dIfmAEceFqB7bhfN3am59lCtDXrCtwH2k1GBA==";

        let global = global_args();
        let locator = setup_locator();
        let cmd = super::Cmd {
            message: Some(message),
            base64: false,
            raw: false,
            signature: signature.to_string(),
            public_key: TEST_PUBLIC_KEY.to_string(),
            hd_path: None,
            locator: locator.clone(),
        };
        let successful = cmd.run(&global);
        assert!(successful.is_ok());
    }

    #[test]
    fn test_verify_base64() {
        // SEP-53 - test case 3
        let message = "2zZDP1sa1BVBfLP7TeeMk3sUbaxAkUhBhDiNdrksaFo=".to_string();
        let signature = "VA1+7hefNwv2NKScH6n+Sljj15kLAge+M2wE7fzFOf+L0MMbssA1mwfJZRyyrhBORQRle10X1Dxpx+UOI4EbDQ==";

        let global = global_args();
        let locator = setup_locator();
        let cmd = super::Cmd {
            message: Some(message),
            base64: true,
            raw: false,
            signature: signature.to_string(),
            public_key: TEST_PUBLIC_KEY.to_string(),
            hd_path: None,
            locator: locator.clone(),
        };
        let successful = cmd.run(&global);
        assert!(successful.is_ok());
    }

    #[test]
    fn test_verify_bad_signature_errors() {
        let message = "Hello, World!".to_string();

        let global = global_args();
        let locator = setup_locator();
        let cmd = super::Cmd {
            message: Some(message),
            base64: false,
            raw: false,
            signature: FALSE_SIGNATURE.to_string(),
            public_key: TEST_PUBLIC_KEY.to_string(),
            hd_path: None,
            locator: locator.clone(),
        };
        let successful = cmd.run(&global);
        assert!(successful.is_err());
    }

    #[test]
    fn test_verify_bad_pubkey_errors() {
        let message = "Hello, World!".to_string();
        let signature = "fO5dbYhXUhBMhe6kId/cuVq/AfEnHRHEvsP8vXh03M1uLpi5e46yO2Q8rEBzu3feXQewcQE5GArp88u6ePK6BA==";

        let global = global_args();
        let locator = setup_locator();
        let cmd = super::Cmd {
            message: Some(message),
            base64: false,
            raw: false,
            signature: signature.to_string(),
            public_key: FALSE_PUBLIC_KEY.to_string(),
            hd_path: None,
            locator: locator.clone(),
        };
        let successful = cmd.run(&global);
        assert!(successful.is_err());
    }

    // Public key = GBXFXNDLV4LSWA4VB7YIL5GBD7BVNR22SGBTDKMO2SBZZHDXSKZYCP7L
    const TEST_SECRET_KEY: &str = "SAKICEVQLYWGSOJS4WW7HZJWAHZVEEBS527LHK5V4MLJALYKICQCJXMW";

    /// Hex ed25519 signature of the raw bytes, with no SEP-53 prefix or hashing.
    fn raw_sign_hex(message_bytes: &[u8]) -> String {
        use crate::{config::secret::Secret, utils::into_signing_key};
        use ed25519_dalek::ed25519::signature::Signer as _;
        use std::str::FromStr;
        let secret = Secret::from_str(TEST_SECRET_KEY).unwrap();
        let signing_key = into_signing_key(&secret.private_key(None).unwrap());
        hex::encode(signing_key.sign(message_bytes).to_bytes())
    }

    #[test]
    fn test_verify_raw_round_trip() {
        let message = "challenge-1:abc123";
        let signature = raw_sign_hex(message.as_bytes());

        let cmd = super::Cmd {
            message: Some(message.to_string()),
            base64: false,
            raw: true,
            signature,
            public_key: TEST_PUBLIC_KEY.to_string(),
            hd_path: None,
            locator: setup_locator(),
        };
        assert!(cmd.run(&global_args()).is_ok());
    }

    #[test]
    fn test_verify_raw_rejects_sep53_signature() {
        // A SEP-53 (base64) signature must not validate on the raw path: even if
        // the hex decodes, it was produced over the prefixed+hashed payload.
        let message = "Hello, World!";
        let sep53_base64 = "fO5dbYhXUhBMhe6kId/cuVq/AfEnHRHEvsP8vXh03M1uLpi5e46yO2Q8rEBzu3feXQewcQE5GArp88u6ePK6BA==";
        let sep53_hex = hex::encode(BASE64.decode(sep53_base64).unwrap());

        let cmd = super::Cmd {
            message: Some(message.to_string()),
            base64: false,
            raw: true,
            signature: sep53_hex,
            public_key: TEST_PUBLIC_KEY.to_string(),
            hd_path: None,
            locator: setup_locator(),
        };
        assert!(matches!(
            cmd.run(&global_args()),
            Err(Error::VerificationFailed)
        ));
    }

    #[test]
    fn test_verify_rejects_raw_secret_key_as_public_key() {
        let secret_key = "SBF5HLRREHMS36XZNTUSKZ6FTXDZGNXOHF4EXKUL5UCWZLPBX3NGJ4BH";
        let cmd = super::Cmd {
            message: Some("Hello, World!".to_string()),
            base64: false,
            raw: false,
            signature: "fO5dbYhXUhBMhe6kId/cuVq/AfEnHRHEvsP8vXh03M1uLpi5e46yO2Q8rEBzu3feXQewcQE5GArp88u6ePK6BA==".to_string(),
            public_key: secret_key.to_string(),
            hd_path: None,
            locator: setup_locator(),
        };
        let err = cmd.run(&global_args()).unwrap_err();
        assert!(matches!(
            err,
            Error::Key(crate::config::key::Error::PublicKeyExpected)
        ));
    }

    #[test]
    fn test_verify_rejects_raw_seed_phrase_as_public_key() {
        let seed_phrase =
            "depth decade power loud smile spatial sign movie judge february rate broccoli";
        let cmd = super::Cmd {
            message: Some("Hello, World!".to_string()),
            base64: false,
            raw: false,
            signature: "fO5dbYhXUhBMhe6kId/cuVq/AfEnHRHEvsP8vXh03M1uLpi5e46yO2Q8rEBzu3feXQewcQE5GArp88u6ePK6BA==".to_string(),
            public_key: seed_phrase.to_string(),
            hd_path: None,
            locator: setup_locator(),
        };
        let err = cmd.run(&global_args()).unwrap_err();
        assert!(matches!(
            err,
            Error::Key(crate::config::key::Error::PublicKeyExpected)
        ));
    }

    #[test]
    fn test_verify_rejects_secure_store_as_public_key() {
        let cmd = super::Cmd {
            message: Some("Hello, World!".to_string()),
            base64: false,
            raw: false,
            signature: "fO5dbYhXUhBMhe6kId/cuVq/AfEnHRHEvsP8vXh03M1uLpi5e46yO2Q8rEBzu3feXQewcQE5GArp88u6ePK6BA==".to_string(),
            public_key: "secure_store:org.stellar.cli-alice".to_string(),
            hd_path: None,
            locator: setup_locator(),
        };
        let err = cmd.run(&global_args()).unwrap_err();
        assert!(matches!(
            err,
            Error::Key(crate::config::key::Error::PublicKeyExpected)
        ));
    }
}
