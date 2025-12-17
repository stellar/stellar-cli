use predicates::prelude::predicate;
use soroban_cli::xdr::{self, Limits, ReadXdr};
use soroban_test::{AssertExt, TestEnv};

use super::util::deploy_hello;

fn get_inclusion_fee_from_xdr(tx_xdr: &str) -> u32 {
    let tx = xdr::TransactionEnvelope::from_xdr_base64(tx_xdr, Limits::none()).unwrap();
    match tx {
        xdr::TransactionEnvelope::TxV0(te) => te.tx.fee,
        xdr::TransactionEnvelope::Tx(te) => te.tx.fee,
        xdr::TransactionEnvelope::TxFeeBump(te) => te.tx.fee.try_into().unwrap(),
    }
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
