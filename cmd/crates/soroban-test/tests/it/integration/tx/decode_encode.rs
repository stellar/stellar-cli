use soroban_cli::xdr::{Limits, ReadXdr, TransactionEnvelope};
use soroban_test::{AssertExt, TestEnv};

use crate::integration::util::test_address;

#[tokio::test]
async fn tx_decode() {
    let sandbox = &TestEnv::new();
    let test_account = test_address(sandbox);

    // Create a simple payment transaction XDR using tx new command
    let tx_xdr = sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "payment",
            "--destination",
            &test_account,
            "--amount",
            "10000000", // 1 XLM in stroops
            "--build-only",
        ])
        .assert()
        .success()
        .stdout_as_str();

    // Decode the XDR to JSON
    let decoded_json = sandbox
        .new_assert_cmd("tx")
        .arg("decode")
        .write_stdin(tx_xdr.as_bytes())
        .assert()
        .success()
        .stdout_as_str();

    // Verify we got valid JSON by parsing it
    let _: serde_json::Value = serde_json::from_str(&decoded_json).unwrap();

    // Verify the decoded JSON contains expected fields
    assert!(decoded_json.contains("tx"));
    assert!(decoded_json.contains("operations"));
    assert!(decoded_json.contains("signatures"));
}

#[tokio::test]
async fn tx_encode() {
    let sandbox = &TestEnv::new();
    let test_account = test_address(sandbox);

    // Create a simple payment transaction XDR using tx new command
    let original_tx_xdr = sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "payment",
            "--destination",
            &test_account,
            "--amount",
            "10000000", // 1 XLM in stroops
            "--build-only",
        ])
        .assert()
        .success()
        .stdout_as_str();

    // Decode the XDR to JSON
    let decoded_json = sandbox
        .new_assert_cmd("tx")
        .arg("decode")
        .write_stdin(original_tx_xdr.as_bytes())
        .assert()
        .success()
        .stdout_as_str();

    // Encode the JSON back to XDR
    let encoded_xdr = sandbox
        .new_assert_cmd("tx")
        .arg("encode")
        .write_stdin(decoded_json.as_bytes())
        .assert()
        .success()
        .stdout_as_str();

    // Verify the round-trip: original XDR should match the re-encoded XDR
    let original_env =
        TransactionEnvelope::from_xdr_base64(&original_tx_xdr, Limits::none()).unwrap();
    let encoded_env = TransactionEnvelope::from_xdr_base64(&encoded_xdr, Limits::none()).unwrap();

    assert_eq!(original_env, encoded_env);
}

#[tokio::test]
async fn tx_decode_invalid_xdr() {
    let sandbox = &TestEnv::new();

    // Try to decode invalid XDR
    sandbox
        .new_assert_cmd("tx")
        .arg("decode")
        .write_stdin("invalid_xdr_data".as_bytes())
        .assert()
        .failure()
        .stderr(predicates::str::contains("error"));
}

#[tokio::test]
async fn tx_encode_invalid_json() {
    let sandbox = &TestEnv::new();

    // Try to encode invalid JSON
    sandbox
        .new_assert_cmd("tx")
        .arg("encode")
        .write_stdin("invalid json data".as_bytes())
        .assert()
        .failure()
        .stderr(predicates::str::contains("error"));
}
