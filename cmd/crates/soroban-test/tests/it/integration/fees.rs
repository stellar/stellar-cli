use predicates::prelude::predicate;
use soroban_cli::xdr::{self, Limits, ReadXdr};
use soroban_rpc::GetFeeStatsResponse;
use soroban_test::{AssertExt, TestEnv};

use super::util::{deploy_hello, HELLO_WORLD};

fn get_inclusion_fee_from_xdr(tx_xdr: &str) -> u32 {
    let tx = xdr::TransactionEnvelope::from_xdr_base64(tx_xdr, Limits::none()).unwrap();
    match tx {
        xdr::TransactionEnvelope::TxV0(te) => te.tx.fee,
        xdr::TransactionEnvelope::Tx(te) => te.tx.fee,
        xdr::TransactionEnvelope::TxFeeBump(te) => te.tx.fee.try_into().unwrap(),
    }
}

#[tokio::test]
async fn fee_stats_text_output() {
    let sandbox = &TestEnv::new();
    sandbox
        .new_assert_cmd("fees")
        .arg("stats")
        .arg("--output")
        .arg("text")
        .assert()
        .success()
        .stdout(predicates::str::contains("Max Soroban Inclusion Fee:"))
        .stdout(predicates::str::contains("Max Inclusion Fee:"))
        .stdout(predicates::str::contains("Latest Ledger:"));
}

#[tokio::test]
async fn fee_stats_json_output() {
    let sandbox = &TestEnv::new();
    let output = sandbox
        .new_assert_cmd("fees")
        .arg("stats")
        .arg("--output")
        .arg("json")
        .assert()
        .success()
        .stdout_as_str();
    let fee_stats_response: GetFeeStatsResponse = serde_json::from_str(&output).unwrap();
    assert!(matches!(fee_stats_response, GetFeeStatsResponse { .. }))
}

#[tokio::test]
async fn inclusion_fee_arg() {
    let sandbox = &TestEnv::new();
    let id = deploy_hello(sandbox).await;

    // Defaults to 100
    let tx_xdr = sandbox
        .new_assert_cmd("contract")
        .arg("invoke")
        .args(["--id", &id.to_string()])
        .arg("--build-only")
        .arg("--")
        .arg("inc")
        .assert()
        .success()
        .stdout_as_str();
    assert_eq!(get_inclusion_fee_from_xdr(&tx_xdr), 100u32);

    // Update manually to 200
    sandbox
        .new_assert_cmd("fees")
        .arg("use")
        .args(["--amount", "200"])
        .assert()
        .stderr(predicate::str::contains(
            "The default inclusion fee is set to `200`",
        ))
        .success();

    let tx_xdr = sandbox
        .new_assert_cmd("contract")
        .arg("invoke")
        .args(["--id", &id.to_string()])
        .arg("--build-only")
        .arg("--")
        .arg("inc")
        .assert()
        .success()
        .stdout_as_str();
    assert_eq!(get_inclusion_fee_from_xdr(&tx_xdr), 200u32);

    // Arg overrides config
    let tx_xdr = sandbox
        .new_assert_cmd("contract")
        .arg("invoke")
        .args(["--id", &id.to_string()])
        .args(["--inclusion-fee", "300"])
        .arg("--build-only")
        .arg("--")
        .arg("inc")
        .assert()
        .success()
        .stdout_as_str();
    assert_eq!(get_inclusion_fee_from_xdr(&tx_xdr), 300u32);

    // Update from fee stats (going to be 100 since sandbox)
    sandbox
        .new_assert_cmd("fees")
        .arg("use")
        .args(["--fee-metric", "p50"])
        .assert()
        .stderr(predicate::str::contains(
            "The default inclusion fee is set to `100`",
        ))
        .success();

    // Deprecated fee arg ignored if inclusion-fee config exists
    let tx_xdr = sandbox
        .new_assert_cmd("contract")
        .arg("invoke")
        .args(["--id", &id.to_string()])
        .args(["--fee", "300"])
        .arg("--build-only")
        .arg("--")
        .arg("inc")
        .assert()
        .success()
        .stdout_as_str();
    assert_eq!(get_inclusion_fee_from_xdr(&tx_xdr), 100u32);

    // Update manually to 200
    sandbox
        .new_assert_cmd("fees")
        .arg("use")
        .args(["--amount", "200"])
        .assert()
        .stderr(predicate::str::contains(
            "The default inclusion fee is set to `200`",
        ))
        .success();

    let tx_xdr = sandbox
        .new_assert_cmd("contract")
        .arg("invoke")
        .args(["--id", &id.to_string()])
        .arg("--build-only")
        .arg("--")
        .arg("inc")
        .assert()
        .success()
        .stdout_as_str();
    assert_eq!(get_inclusion_fee_from_xdr(&tx_xdr), 200u32);

    // Verify unset clears the config
    sandbox
        .new_assert_cmd("fees")
        .arg("unset")
        .assert()
        .stderr(predicate::str::contains(
            "The default inclusion fee has been cleared",
        ))
        .success();

    let tx_xdr = sandbox
        .new_assert_cmd("contract")
        .arg("invoke")
        .args(["--id", &id.to_string()])
        .arg("--build-only")
        .arg("--")
        .arg("inc")
        .assert()
        .success()
        .stdout_as_str();
    assert_eq!(get_inclusion_fee_from_xdr(&tx_xdr), 100u32);
}

#[tokio::test]
async fn large_fee_transactions_use_fee_bump() {
    let sandbox = &TestEnv::new();

    // install HELLO_WORLD
    // don't test fee bump here as other integration tests upload WASMs, so this
    // might be a no-op
    let wasm_hash = sandbox
        .new_assert_cmd("contract")
        .arg("upload")
        .arg("--wasm")
        .arg(HELLO_WORLD.path().to_string_lossy().to_string())
        .assert()
        .success()
        .stdout_as_str();

    // deploy HELLO_WORLD with a high inclusion fee to trigger fee-bump wrapping
    let id = sandbox
        .new_assert_cmd("contract")
        .arg("deploy")
        .args(["--wasm-hash", wasm_hash.trim()])
        .args(["--inclusion-fee", &(u32::MAX - 50).to_string()])
        .assert()
        .success()
        .stdout_as_str();

    // invoke HELLO_WORLD with a high resource fee to trigger fee-bump wrapping
    let std_err = sandbox
        .new_assert_cmd("contract")
        .arg("invoke")
        .args(["--id", &id.to_string()])
        .args(["--resource-fee", &(u64::from(u32::MAX) + 1).to_string()])
        .arg("--")
        .arg("inc")
        .assert()
        .success()
        .stderr_as_str();

    // validate log output indicates fee bump was used
    assert!(std_err.contains("Signing fee bump transaction"));
}
