use rand::Rng;

use soroban_cli::xdr::{Limits, ReadXdr, TransactionEnvelope, Memo};
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
    let updated_tx = sandbox
        .new_assert_cmd("tx")
        .arg("edit")
        .arg("source-account")
        .arg("set")
        .arg(&new_address)
        .write_stdin(tx_base64.as_bytes())
        .assert()
        .success()
        .stdout_as_str();
    let tx_env_two = TransactionEnvelope::from_xdr_base64(&updated_tx, Limits::none()).unwrap();
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
    let tx_env = TransactionEnvelope::from_xdr_base64(&tx_base64, Limits::none()).unwrap();
    let tx = soroban_cli::commands::tx::xdr::unwrap_envelope_v1(tx_env).unwrap();

    let test_seq_num = 2;
    let updated_tx = sandbox
        .new_assert_cmd("tx")
        .arg("edit")
        .arg("seq-num")
        .arg("set")
        .arg(2.to_string())
        .write_stdin(tx_base64.as_bytes())
        .assert()
        .success()
        .stdout_as_str();
    let tx_env = TransactionEnvelope::from_xdr_base64(&updated_tx, Limits::none()).unwrap();
    let tx = soroban_cli::commands::tx::xdr::unwrap_envelope_v1(tx_env).unwrap();
    assert_eq!(tx.seq_num, soroban_cli::xdr::SequenceNumber(test_seq_num));
}

#[test]
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
    let updated_tx = sandbox
        .new_assert_cmd("tx")
        .arg("edit")
        .arg("fee")
        .arg("set")
        .arg(test_fee.to_string())
        .write_stdin(tx_base64.as_bytes())
        .assert()
        .success()
        .stdout_as_str();
    let tx_env = TransactionEnvelope::from_xdr_base64(&updated_tx, Limits::none()).unwrap();
    let tx = soroban_cli::commands::tx::xdr::unwrap_envelope_v1(tx_env).unwrap();
    assert_eq!(tx.fee, test_fee);
}

#[test]
//tx edit memo set text <MEMO_TEXT>
fn memo_set_text() {
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
    let tx_env = TransactionEnvelope::from_xdr_base64(&tx_base64, Limits::none()).unwrap();
    let tx = soroban_cli::commands::tx::xdr::unwrap_envelope_v1(tx_env).unwrap();
    assert_eq!(tx.memo, Memo::None);

    let test_memo_text = "memo text";
    let updated_tx = sandbox
        .new_assert_cmd("tx")
        .arg("edit")
        .arg("memo")
        .arg("set")
        .arg("text")
        .arg(test_memo_text)
        .write_stdin(tx_base64.as_bytes())
        .assert()
        .success()
        .stdout_as_str();
    let tx_env = TransactionEnvelope::from_xdr_base64(&updated_tx, Limits::none()).unwrap();
    let tx = soroban_cli::commands::tx::xdr::unwrap_envelope_v1(tx_env).unwrap();
    assert_eq!(get_memo_value(tx.memo), test_memo_text);
}

#[test]
//tx edit memo set id <MEMO_ID>
fn memo_set_id() {
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
    let tx_env = TransactionEnvelope::from_xdr_base64(&tx_base64, Limits::none()).unwrap();
    let tx = soroban_cli::commands::tx::xdr::unwrap_envelope_v1(tx_env).unwrap();
    assert_eq!(tx.memo, Memo::None);

    let test_memo_id = "19";
    let updated_tx = sandbox
        .new_assert_cmd("tx")
        .arg("edit")
        .arg("memo")
        .arg("set")
        .arg("id")
        .arg(test_memo_id)
        .write_stdin(tx_base64.as_bytes())
        .assert()
        .success()
        .stdout_as_str();
    let tx_env = TransactionEnvelope::from_xdr_base64(&updated_tx, Limits::none()).unwrap();
    let tx = soroban_cli::commands::tx::xdr::unwrap_envelope_v1(tx_env).unwrap();
    assert_eq!(get_memo_value(tx.memo), test_memo_id);
}

#[test]
//tx edit memo set hash <HASH>
fn memo_set_hash() {
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
    let tx_env = TransactionEnvelope::from_xdr_base64(&tx_base64, Limits::none()).unwrap();
    let tx = soroban_cli::commands::tx::xdr::unwrap_envelope_v1(tx_env).unwrap();
    assert_eq!(tx.memo, Memo::None);

    let mut rng = rand::thread_rng();
    let test_memo_hash: [u8; 32] = rng.gen();
    let test_memo_hash = hex::encode(test_memo_hash);

    let updated_tx = sandbox
        .new_assert_cmd("tx")
        .arg("edit")
        .arg("memo")
        .arg("set")
        .arg("hash")
        .arg(&test_memo_hash)
        .write_stdin(tx_base64.as_bytes())
        .assert()
        .success()
        .stdout_as_str();
    let tx_env = TransactionEnvelope::from_xdr_base64(&updated_tx, Limits::none()).unwrap();
    let tx = soroban_cli::commands::tx::xdr::unwrap_envelope_v1(tx_env).unwrap();
    assert_eq!(get_memo_value(tx.memo), test_memo_hash);
}

#[test]
//tx edit memo set return <RETURN>
fn memo_set_return() {
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
    let tx_env = TransactionEnvelope::from_xdr_base64(&tx_base64, Limits::none()).unwrap();
    let tx = soroban_cli::commands::tx::xdr::unwrap_envelope_v1(tx_env).unwrap();
    assert_eq!(tx.memo, Memo::None);

    let mut rng = rand::thread_rng();
    let test_memo_return: [u8; 32] = rng.gen();
    let test_memo_return = hex::encode(test_memo_return);

    let updated_tx = sandbox
        .new_assert_cmd("tx")
        .arg("edit")
        .arg("memo")
        .arg("set")
        .arg("return")
        .arg(&test_memo_return)
        .write_stdin(tx_base64.as_bytes())
        .assert()
        .success()
        .stdout_as_str();
    let tx_env = TransactionEnvelope::from_xdr_base64(&updated_tx, Limits::none()).unwrap();
    let tx = soroban_cli::commands::tx::xdr::unwrap_envelope_v1(tx_env).unwrap();
    assert_eq!(get_memo_value(tx.memo), test_memo_return);
}

#[test]
//tx edit memo set return <RETURN>
fn memo_clear() {
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

    let mut rng = rand::thread_rng();
    let test_memo_return: [u8; 32] = rng.gen();
    let test_memo_return = hex::encode(test_memo_return);

    let updated_tx = sandbox
        .new_assert_cmd("tx")
        .arg("edit")
        .arg("memo")
        .arg("set")
        .arg("return")
        .arg(&test_memo_return)
        .write_stdin(tx_base64.as_bytes())
        .assert()
        .success()
        .stdout_as_str();
    let tx_env = TransactionEnvelope::from_xdr_base64(&updated_tx, Limits::none()).unwrap();
    let tx = soroban_cli::commands::tx::xdr::unwrap_envelope_v1(tx_env).unwrap();
    assert_eq!(get_memo_value(tx.memo), test_memo_return);

    let clear_tx = sandbox
        .new_assert_cmd("tx")
        .arg("edit")
        .arg("memo")
        .arg("clear")
        .write_stdin(tx_base64.as_bytes())
        .assert()
        .success()
        .stdout_as_str();
    let tx_env = TransactionEnvelope::from_xdr_base64(&clear_tx, Limits::none()).unwrap();
    let tx = soroban_cli::commands::tx::xdr::unwrap_envelope_v1(tx_env).unwrap();
    assert_eq!(tx.memo, Memo::None);
}

fn get_memo_value(memo: Memo) -> String {
    match &memo {
        Memo::None => "".to_string(),
        Memo::Text(text) => String::from_utf8_lossy(text).to_string(),
        Memo::Id(id) => id.to_string(),
        Memo::Hash(hash) => hash.to_string(),
        Memo::Return(hash) => hash.to_string(),
    }
}
