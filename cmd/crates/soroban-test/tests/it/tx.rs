use soroban_test::{AssertExt, TestEnv};

// Transaction envelope from https://github.com/stellar/stellar-cli/issues/2455.
const TX_ENVELOPE: &str = "AAAAAgAAAAAnEWb4nxMsQhdnS16MqBxwItF3X/JNRkfxu8eyEQyfegAAAGQAAAAAAAAAAQAAAAAAAAAAAAAAAQAAAAAAAAABAAAAABk++lrW4HlZwaWwYdYvJPCil1ibyI/VTH6WKda2sOC+AAAAAAAAAAAAAABkAAAAAAAAAAA=";
const SECRET_KEY: &str = "SAKICEVQLYWGSOJS4WW7HZJWAHZVEEBS527LHK5V4MLJALYKICQCJXMW";

// `tx sign` only mixes the network passphrase into the transaction hash, so it
// must not require an RPC URL (regression test for #2455).
#[tokio::test]
async fn tx_sign_requires_only_network_passphrase() {
    let sandbox = &TestEnv::new();

    let output = sandbox
        .new_assert_cmd("tx")
        .args([
            "sign",
            TX_ENVELOPE,
            "--network-passphrase",
            "specified manually",
            "--sign-with-key",
            SECRET_KEY,
        ])
        .assert()
        .success()
        .stdout_as_str();

    // A signed envelope is longer than the unsigned one it was built from.
    assert!(output.trim().len() > TX_ENVELOPE.len());
}

// Same expectation for `tx hash`, which also only needs the passphrase.
#[tokio::test]
async fn tx_hash_requires_only_network_passphrase() {
    let sandbox = &TestEnv::new();

    let output = sandbox
        .new_assert_cmd("tx")
        .args([
            "hash",
            TX_ENVELOPE,
            "--network-passphrase",
            "specified manually",
        ])
        .assert()
        .success()
        .stdout_as_str();

    let hash = output.trim();
    assert_eq!(hash.len(), 64, "expected a 32-byte hex hash, got: {hash}");
    assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
}
