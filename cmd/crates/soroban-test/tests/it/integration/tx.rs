use soroban_sdk::xdr::{Limits, ReadXdr, TransactionEnvelope, WriteXdr};
use soroban_test::{AssertExt, TestEnv};

use crate::integration::util::{deploy_contract, deploy_hello, DeployKind, HELLO_WORLD};

#[tokio::test]
async fn simulate() {
    let sandbox = &TestEnv::new();
    let xdr_base64_build_only = deploy_contract(sandbox, HELLO_WORLD, DeployKind::BuildOnly).await;
    let xdr_base64_sim_only = deploy_contract(sandbox, HELLO_WORLD, DeployKind::SimOnly).await;
    let tx_env =
        TransactionEnvelope::from_xdr_base64(&xdr_base64_build_only, Limits::none()).unwrap();
    let tx = soroban_cli::commands::tx::xdr::unwrap_envelope_v1(tx_env).unwrap();
    let assembled_str = sandbox
        .new_assert_cmd("tx")
        .arg("simulate")
        .write_stdin(xdr_base64_build_only.as_bytes())
        .assert()
        .success()
        .stdout_as_str();
    assert_eq!(xdr_base64_sim_only, assembled_str);
    let assembled = sandbox
        .client()
        .simulate_and_assemble_transaction(&tx)
        .await
        .unwrap();
    let txn_env: TransactionEnvelope = assembled.transaction().clone().into();
    assert_eq!(
        txn_env.to_xdr_base64(Limits::none()).unwrap(),
        assembled_str
    );
}

#[tokio::test]
async fn send() {
    let sandbox = &TestEnv::new();
    sandbox
        .new_assert_cmd("contract")
        .arg("install")
        .args(["--wasm", HELLO_WORLD.path().as_os_str().to_str().unwrap()])
        .assert()
        .success();

    let xdr_base64 = deploy_contract(sandbox, HELLO_WORLD, DeployKind::SimOnly).await;
    println!("{xdr_base64}");
    let tx_env = TransactionEnvelope::from_xdr_base64(&xdr_base64, Limits::none()).unwrap();
    let tx_env = sign_manually(sandbox, &tx_env);

    println!(
        "Transaction to send:\n{}",
        tx_env.to_xdr_base64(Limits::none()).unwrap()
    );

    let assembled_str = sandbox
        .new_assert_cmd("tx")
        .arg("send")
        .write_stdin(tx_env.to_xdr_base64(Limits::none()).unwrap())
        .assert()
        .success()
        .stdout_as_str();
    println!("Transaction sent: {assembled_str}");
}

fn sign_manually(sandbox: &TestEnv, tx_env: &TransactionEnvelope) -> TransactionEnvelope {
    TransactionEnvelope::from_xdr_base64(
        sandbox
            .new_assert_cmd("tx")
            .arg("sign")
            .arg("--source=test")
            .write_stdin(tx_env.to_xdr_base64(Limits::none()).unwrap().as_bytes())
            .assert()
            .success()
            .stdout_as_str(),
        Limits::none(),
    )
    .unwrap()
}

#[tokio::test]
async fn sign() {
    let sandbox = &TestEnv::new();
    let id = &deploy_hello(sandbox).await;
    // Create new test_other account
    sandbox
        .new_assert_cmd("keys")
        .arg("generate")
        .arg("test_other")
        .assert();

    // Get Xdr for transaction where auth is required for test_other
    let xdr_base64 = sandbox
        .new_assert_cmd("contract")
        .arg("invoke")
        .arg("--id")
        .arg(id)
        .arg("--sim-only")
        .arg("--")
        .arg("auth")
        .arg("--world=world")
        .arg("--addr=test_other")
        .assert()
        .success()
        .stdout_as_str();
    // Sign the transaction's auth entry with test_other
    let xdr_base64 = sandbox
        .new_assert_cmd("tx")
        .arg("sign")
        .arg("--auth")
        .arg("--source=test_other")
        .write_stdin(xdr_base64.as_bytes())
        .assert()
        .success()
        .stdout_as_str();
    // Sign the transaction with test as source account
    let xdr_base64 = sandbox
        .new_assert_cmd("tx")
        .arg("sign")
        .write_stdin(xdr_base64.as_bytes())
        .assert()
        .success()
        .stdout_as_str();
    // Send transaction
    sandbox
        .new_assert_cmd("tx")
        .arg("send")
        .write_stdin(xdr_base64.as_bytes())
        .assert()
        .success()
        .stdout(predicates::str::contains(r#""status": "SUCCESS""#));
}
