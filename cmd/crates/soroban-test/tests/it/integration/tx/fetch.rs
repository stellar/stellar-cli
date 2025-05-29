use soroban_cli::{
    utils::transaction_hash,
    xdr::{
        Limits, ReadXdr, TransactionEnvelope, TransactionMeta, TransactionResult,
        TransactionResultExt, TransactionResultResult, TransactionV1Envelope,
    },
};

use soroban_test::{AssertExt, TestEnv};

#[tokio::test]
async fn tx_fetch() {
    let sandbox = &TestEnv::new();
    let test_account_alias = "test";
    // create a tx
    let data_name = "test_data_key";
    let data_value = "abcdef";
    let tx_hash = add_account_data(sandbox, test_account_alias, data_name, data_value).await;

    // fetch the tx result
    let output = sandbox
        .new_assert_cmd("tx")
        .arg("fetch")
        .arg("result")
        .arg("--hash")
        .arg(&tx_hash)
        .arg("--network")
        .arg("testnet")
        .assert()
        .success()
        .stdout_as_str();

    let parsed: TransactionResult = serde_json::from_str(&output).unwrap();
    assert_eq!(parsed.fee_charged, 100);
    assert!(matches!(
        parsed.result,
        TransactionResultResult::TxSuccess { .. }
    ));
    assert_eq!(parsed.ext, TransactionResultExt::V0);

    // fetch the tx meta
    let output = sandbox
        .new_assert_cmd("tx")
        .arg("fetch")
        .arg("meta")
        .arg("--hash")
        .arg(&tx_hash)
        .arg("--network")
        .arg("testnet")
        .assert()
        .success()
        .stdout_as_str();

    let parsed: TransactionMeta = serde_json::from_str(&output).unwrap();
    assert!(matches!(parsed, TransactionMeta::V3 { .. }));

    // fetch the tx envelope
    let output = sandbox
        .new_assert_cmd("tx")
        .arg("fetch")
        .arg("envelope")
        .arg("--hash")
        .arg(&tx_hash)
        .arg("--network")
        .arg("testnet")
        .assert()
        .success()
        .stdout_as_str();

    let parsed: TransactionEnvelope = serde_json::from_str(&output).unwrap();
    assert!(matches!(
        parsed,
        TransactionEnvelope::Tx(TransactionV1Envelope { .. })
    ));
}

#[tokio::test]
async fn tx_fetch_default_to_envelope() {
    let sandbox = &TestEnv::new();
    let test_account_alias = "test";
    // create a tx
    let data_name = "test_data_key";
    let data_value = "abcdef";
    let tx_hash = add_account_data(sandbox, test_account_alias, data_name, data_value).await;

    // fetch the tx envelope when no subcommand is given
    let output = sandbox
        .new_assert_cmd("tx")
        .arg("fetch")
        .arg("--hash")
        .arg(&tx_hash)
        .arg("--network")
        .arg("testnet")
        .assert()
        .success()
        .stdout_as_str();

    let parsed: TransactionEnvelope = serde_json::from_str(&output).unwrap();
    assert!(matches!(
        parsed,
        TransactionEnvelope::Tx(TransactionV1Envelope { .. })
    ));
}

#[tokio::test]
async fn tx_fetch_xdr_output() {
    let sandbox = &TestEnv::new();
    let test_account_alias = "test";
    // create a tx
    let data_name = "test_data_key";
    let data_value = "abcdef";
    let tx_hash = add_account_data(sandbox, test_account_alias, data_name, data_value).await;

    // fetch the tx result
    let output = sandbox
        .new_assert_cmd("tx")
        .arg("fetch")
        .arg("result")
        .arg("--hash")
        .arg(&tx_hash)
        .arg("--network")
        .arg("testnet")
        .arg("--output")
        .arg("xdr")
        .assert()
        .success()
        .stdout_as_str();

    let parsed_xdr = TransactionResult::from_xdr_base64(output, Limits::none()).unwrap();
    assert_eq!(parsed_xdr.fee_charged, 100);
    assert!(matches!(
        parsed_xdr.result,
        TransactionResultResult::TxSuccess { .. }
    ));
    assert_eq!(parsed_xdr.ext, TransactionResultExt::V0);

    // fetch the tx meta
    let output = sandbox
        .new_assert_cmd("tx")
        .arg("fetch")
        .arg("meta")
        .arg("--hash")
        .arg(&tx_hash)
        .arg("--network")
        .arg("testnet")
        .arg("--output")
        .arg("xdr")
        .assert()
        .success()
        .stdout_as_str();

    let parsed_xdr = TransactionMeta::from_xdr_base64(output, Limits::none()).unwrap();
    assert!(matches!(parsed_xdr, TransactionMeta::V3 { .. }));

    // fetch the tx envelope
    let output = sandbox
        .new_assert_cmd("tx")
        .arg("fetch")
        .arg("envelope")
        .arg("--hash")
        .arg(&tx_hash)
        .arg("--network")
        .arg("testnet")
        .arg("--output")
        .arg("xdr")
        .assert()
        .success()
        .stdout_as_str();

    let parsed_xdr = TransactionEnvelope::from_xdr_base64(&output, Limits::none()).unwrap();
    assert!(matches!(
        parsed_xdr,
        TransactionEnvelope::Tx(TransactionV1Envelope { .. })
    ));
}

async fn add_account_data(
    sandbox: &TestEnv,
    account_alias: &str,
    key: &str,
    value: &str,
) -> String {
    let tx_xdr = sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "manage-data",
            "--data-name",
            key,
            "--data-value",
            value,
            "--source",
            account_alias,
            "--build-only",
        ])
        .assert()
        .success()
        .stdout_as_str();

    let tx_env = TransactionEnvelope::from_xdr_base64(tx_xdr.clone(), Limits::none()).unwrap();
    let tx = if let TransactionEnvelope::Tx(env) = tx_env {
        env.tx
    } else {
        panic!("Expected TransactionEnvelope::Tx, got something else");
    };

    // submit the tx
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "manage-data",
            "--data-name",
            key,
            "--data-value",
            value,
            "--source",
            account_alias,
        ])
        .assert()
        .success()
        .stdout_as_str();

    hex::encode(transaction_hash(&tx, &sandbox.network.network_passphrase).unwrap())
}
