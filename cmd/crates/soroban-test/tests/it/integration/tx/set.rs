use soroban_cli::xdr::{Limits, ReadXdr, TransactionEnvelope};
use soroban_test::{AssertExt, TestEnv};

use crate::integration::util::HELLO_WORLD;

//todo: pull these out into a helper to use with operations too
pub fn test_address(sandbox: &TestEnv) -> String {
    sandbox
        .new_assert_cmd("keys")
        .arg("address")
        .arg("test")
        .assert()
        .success()
        .stdout_as_str()
}

fn new_account(sandbox: &TestEnv, name: &str) -> String {
    sandbox.generate_account(name, None).assert().success();
    sandbox
        .new_assert_cmd("keys")
        .args(["address", name])
        .assert()
        .success()
        .stdout_as_str()
}

#[test]
//tx edit source-account set <SOURCE_ACCOUNT>
fn source_account_set() {
    let sandbox = &TestEnv::new();
    let test_address = test_address(sandbox); // this returns the address for the account with alias "test"
    let new_address = new_account(sandbox, "new_account");

    let tx_base64 = sandbox
        .new_assert_cmd("contract")
        .arg("install")
        .args([
            "--source",
            "test",
            "--wasm",
            HELLO_WORLD.path().as_os_str().to_str().unwrap(),
            "--build-only",
        ])
        .assert()
        .success()
        .stdout_as_str();
    let tx_env = TransactionEnvelope::from_xdr_base64(&tx_base64, Limits::none()).unwrap();
    let tx_before = soroban_cli::commands::tx::xdr::unwrap_envelope_v1(tx_env).unwrap();
    assert_eq!(tx_before.source_account.to_string(), test_address);

    // change transaction source account
    let new_tx = sandbox
        .new_assert_cmd("tx")
        .arg("edit")
        .arg("source-account")
        .arg("set")
        .arg(&new_address)
        .write_stdin(tx_base64.as_bytes())
        .assert()
        .success()
        .stdout_as_str();
    let tx_env_two = TransactionEnvelope::from_xdr_base64(&new_tx, Limits::none()).unwrap();
    let tx = soroban_cli::commands::tx::xdr::unwrap_envelope_v1(tx_env_two).unwrap();
    assert_eq!(tx.source_account.to_string(), new_address);
}

#[test]
//tx edit sequence-number set <SEQUENCE_NUMBER>
fn seq_num_set() {
    let sandbox = &TestEnv::new();
    let tx_base64 = sandbox
        .new_assert_cmd("contract")
        .arg("install")
        .args([
            "--wasm",
            HELLO_WORLD.path().as_os_str().to_str().unwrap(),
            "--build-only",
        ])
        .assert()
        .success()
        .stdout_as_str();

    let test_seq_num = 2;
    let new_tx = sandbox
        .new_assert_cmd("tx")
        .arg("edit")
        .arg("seq-num")
        .arg("set")
        .arg(2.to_string())
        .write_stdin(tx_base64.as_bytes())
        .assert()
        .success()
        .stdout_as_str();
    let tx_env = TransactionEnvelope::from_xdr_base64(&new_tx, Limits::none()).unwrap();
    let tx = soroban_cli::commands::tx::xdr::unwrap_envelope_v1(tx_env).unwrap();
    assert_eq!(tx.seq_num, soroban_cli::xdr::SequenceNumber(test_seq_num));
}

#[test]
#[ignore]
//tx edit fee set <FEE>
fn fee_set() {
    let sandbox = &TestEnv::new();
    let tx_base64 = sandbox
        .new_assert_cmd("contract")
        .arg("install")
        .args([
            "--wasm",
            HELLO_WORLD.path().as_os_str().to_str().unwrap(),
            "--build-only",
        ])
        .assert()
        .success()
        .stdout_as_str();

    let test_fee = 1000;
    let new_tx = sandbox
        .new_assert_cmd("tx")
        .arg("edit")
        .arg("fee")
        .arg("set")
        .arg(test_fee.to_string())
        .write_stdin(tx_base64.as_bytes())
        .assert()
        .success()
        .stdout_as_str();
    let tx_env = TransactionEnvelope::from_xdr_base64(&new_tx, Limits::none()).unwrap();
    let tx = soroban_cli::commands::tx::xdr::unwrap_envelope_v1(tx_env).unwrap();
    assert_eq!(tx.fee, test_fee);
}
