//! Signer plugin for the `test_ed25519_account` smart account: signs the
//! request's `payload` with the identity named by `args.key`, and returns the
//! signature as an `ScVal::Bytes`.

use std::io::Read;
use std::process::Command;

use base64::Engine as _;
use ed25519_dalek::{Signer as _, SigningKey};

const SCV_BYTES: u32 = 13;

fn main() {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input).unwrap();
    let request: serde_json::Value = serde_json::from_str(&input).unwrap();
    assert_eq!(request["mode"], "sign_auth");

    let key = request["args"]["key"]
        .as_str()
        .expect("args.key is required");
    let stellar = std::env::var("STELLAR_BIN").expect("STELLAR_BIN is set");
    let output = Command::new(stellar)
        .args(["keys", "secret", key])
        .output()
        .unwrap();
    assert!(output.status.success(), "stellar keys secret {key} failed");
    let secret = String::from_utf8(output.stdout).unwrap();
    let secret = stellar_strkey::ed25519::PrivateKey::from_string(secret.trim()).unwrap();

    let payload = hex::decode(request["payload"].as_str().unwrap()).unwrap();
    let signature = SigningKey::from_bytes(&secret.0).sign(&payload).to_bytes();

    // XDR for ScVal::Bytes(signature)
    let mut xdr = Vec::with_capacity(72);
    xdr.extend_from_slice(&SCV_BYTES.to_be_bytes());
    xdr.extend_from_slice(&u32::try_from(signature.len()).unwrap().to_be_bytes());
    xdr.extend_from_slice(&signature);
    println!("{}", base64::engine::general_purpose::STANDARD.encode(xdr));
}
