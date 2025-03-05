use rand::Rng;

use soroban_cli::xdr::{Limits, Memo, Preconditions, ReadXdr, TimeBounds, TransactionEnvelope};
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

fn test_tx_string(sandbox: &TestEnv) -> String {
    sandbox
    .new_assert_cmd("contract")
    .arg("install")
    .args([
        "--wasm",
        HELLO_WORLD.path().as_os_str().to_str().unwrap(),
        "--build-only",
    ])
    .assert()
    .success()
    .stdout_as_str()
}

#[tokio::test]
//tx edit source-account set <SOURCE_ACCOUNT>
async fn source_account_set() {
    let sandbox = &TestEnv::new();
    let test_address = test_address(sandbox); // this returns the address for the account with alias "test"
    let new_address = new_account(sandbox, "new_account");

    let tx_base64 = test_tx_string(sandbox);
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

#[tokio::test]
//tx edit sequence-number set <SEQUENCE_NUMBER>
async fn seq_num_set() {
    let sandbox = &TestEnv::new();
    let tx_base64 = test_tx_string(sandbox);
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

#[tokio::test]
//tx edit fee set <FEE>
async fn fee_set() {
    let sandbox = &TestEnv::new();
    let tx_base64 = test_tx_string(sandbox);
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

#[tokio::test]
//tx edit memo set text <MEMO_TEXT>
async fn memo_set_text() {
    let sandbox = &TestEnv::new();
    let tx_base64 = test_tx_string(sandbox);
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

#[tokio::test]
//tx edit memo set id <MEMO_ID>
async fn memo_set_id() {
    let sandbox = &TestEnv::new();
    let tx_base64 = test_tx_string(sandbox);
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

#[tokio::test]
//tx edit memo set hash <HASH>
async fn memo_set_hash() {
    let sandbox = &TestEnv::new();
    let tx_base64 = test_tx_string(sandbox);
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

#[tokio::test]
//tx edit memo set return <RETURN>
async fn memo_set_return() {
    let sandbox = &TestEnv::new();
    let tx_base64 = test_tx_string(sandbox);
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

#[tokio::test]
//tx edit memo clear
async fn memo_clear() {
    let sandbox = &TestEnv::new();
    let tx_base64 = test_tx_string(sandbox);
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

#[tokio::test]
// setting max when no timebounds are set
async fn time_bounds_max() {
    let sandbox = &TestEnv::new();
    let tx_base64 = test_tx_string(sandbox);
    let tx_env = TransactionEnvelope::from_xdr_base64(&tx_base64, Limits::none()).unwrap();
    let tx = soroban_cli::commands::tx::xdr::unwrap_envelope_v1(tx_env).unwrap();
    assert_eq!(tx.cond, Preconditions::None);

    let max = 200;
    let updated_tx = sandbox
        .new_assert_cmd("tx")
        .arg("edit")
        .arg("time-bound")
        .arg("set")
        .arg("max")
        .arg(max.to_string())
        .write_stdin(tx_base64.as_bytes())
        .assert()
        .success()
        .stdout_as_str();
    let tx_env = TransactionEnvelope::from_xdr_base64(&updated_tx, Limits::none()).unwrap();
    let tx = soroban_cli::commands::tx::xdr::unwrap_envelope_v1(tx_env).unwrap();
    if let Preconditions::V2(preconditions) = &tx.cond {
        assert_eq!(
            preconditions.time_bounds,
            Some(TimeBounds {
                min_time: 0.into(),
                max_time: max.into()
            })
        );
    }
}

#[tokio::test]
// setting min when no time bounds are set
async fn time_bounds_min() {
    let sandbox = &TestEnv::new();
    let tx_base64 = test_tx_string(sandbox);
    let tx_env = TransactionEnvelope::from_xdr_base64(&tx_base64, Limits::none()).unwrap();
    let tx = soroban_cli::commands::tx::xdr::unwrap_envelope_v1(tx_env).unwrap();
    assert_eq!(tx.cond, Preconditions::None);

    let min = 200;
    let updated_tx = sandbox
        .new_assert_cmd("tx")
        .arg("edit")
        .arg("time-bound")
        .arg("set")
        .arg("min")
        .arg(min.to_string())
        .write_stdin(tx_base64.as_bytes())
        .assert()
        .success()
        .stdout_as_str();
    let tx_env = TransactionEnvelope::from_xdr_base64(&updated_tx, Limits::none()).unwrap();
    let tx = soroban_cli::commands::tx::xdr::unwrap_envelope_v1(tx_env).unwrap();
    if let Preconditions::V2(preconditions) = &tx.cond {
        assert_eq!(
            preconditions.time_bounds,
            Some(TimeBounds {
                min_time: min.into(),
                max_time: 0.into()
            })
        );
    } else {
        assert!(false);
    }
}

#[tokio::test]
// resetting time bounds
async fn time_bounds() {
    let sandbox = &TestEnv::new();
    let tx_base64 = test_tx_string(sandbox);
    let tx_env = TransactionEnvelope::from_xdr_base64(&tx_base64, Limits::none()).unwrap();
    let tx = soroban_cli::commands::tx::xdr::unwrap_envelope_v1(tx_env).unwrap();
    assert_eq!(tx.cond, Preconditions::None);

    // set min to 200
    let min = 200;
    let update_min_tx = sandbox
        .new_assert_cmd("tx")
        .arg("edit")
        .arg("time-bound")
        .arg("set")
        .arg("min")
        .arg(min.to_string())
        .write_stdin(tx_base64.as_bytes())
        .assert()
        .success()
        .stdout_as_str();
    let tx_env = TransactionEnvelope::from_xdr_base64(&update_min_tx, Limits::none()).unwrap();
    let tx = soroban_cli::commands::tx::xdr::unwrap_envelope_v1(tx_env).unwrap();
    if let Preconditions::V2(preconditions) = &tx.cond {
        assert_eq!(
            preconditions.time_bounds,
            Some(TimeBounds {min_time: min.into(), max_time: 0.into()}) // check this - should it be max instead?
        );
    } else {
        assert!(false);
    }

    // verify that we can set max without disrupting min
    let max = 500;
    let update_max_tx = sandbox
        .new_assert_cmd("tx")
        .arg("edit")
        .arg("time-bound")
        .arg("set")
        .arg("max")
        .arg(max.to_string())
        .write_stdin(update_min_tx.as_bytes())
        .assert()
        .success()
        .stdout_as_str();
    let tx_env = TransactionEnvelope::from_xdr_base64(&update_max_tx, Limits::none()).unwrap();
    let tx = soroban_cli::commands::tx::xdr::unwrap_envelope_v1(tx_env).unwrap();
    if let Preconditions::V2(preconditions) = &tx.cond {
        assert_eq!(
            preconditions.time_bounds,
            Some(TimeBounds {min_time: min.into(), max_time: max.into()})
        );
    } else {
        assert!(false);
    }

    // verify that we can reset min without disrupting max
    let new_min = 100;
    let new_update_min_tx = sandbox
        .new_assert_cmd("tx")
        .arg("edit")
        .arg("time-bound")
        .arg("set")
        .arg("min")
        .arg(new_min.to_string())
        .write_stdin(update_max_tx.as_bytes())
        .assert()
        .success()
        .stdout_as_str();
    let tx_env = TransactionEnvelope::from_xdr_base64(&new_update_min_tx, Limits::none()).unwrap();
    let tx = soroban_cli::commands::tx::xdr::unwrap_envelope_v1(tx_env).unwrap();
    if let Preconditions::V2(preconditions) = &tx.cond {
        assert_eq!(
            preconditions.time_bounds,
            Some(TimeBounds {min_time: new_min.into(), max_time: max.into()})
        );
    } else {
        assert!(false);
    }
}
