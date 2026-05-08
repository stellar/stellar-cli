use stellar_ledger::Blob;

use soroban_test::{AssertExt, TestEnv, Wasm};
use std::sync::Arc;

use stellar_ledger::emulator_test_support::*;

use soroban_cli::{
    tx::builder::TxExt,
    xdr::{self, Limits, OperationBody, ReadXdr, TransactionEnvelope, WriteXdr},
};

use test_case::test_case;

const HELLO_WORLD: &Wasm = &Wasm::Custom("test-wasms", "test_hello_world");

#[test_case("nanos", 0; "when the device is NanoS")]
#[test_case("nanox", 1; "when the device is NanoX")]
#[test_case("nanosp", 2; "when the device is NanoS Plus")]
#[tokio::test]
async fn test_signer(ledger_device_model: &str, hd_path: u32) {
    let sandbox = Arc::new(TestEnv::new());
    let container = TestEnv::speculos_container(ledger_device_model).await;
    let host_port = container.get_host_port_ipv4(9998).await.unwrap();
    let ui_host_port = container.get_host_port_ipv4(5000).await.unwrap();

    let ledger = ledger(host_port).await;

    let key = ledger.get_public_key(&hd_path.into()).await.unwrap();

    let verifying_key = ed25519_dalek::VerifyingKey::from_bytes(&key.0).unwrap();
    let body: OperationBody =
        (&soroban_cli::commands::tx::new::bump_sequence::Args { bump_to: 100 }).into();
    let operation = xdr::Operation {
        body,
        source_account: None,
    };
    let source_account = xdr::MuxedAccount::Ed25519(key.0.into());
    let tx_env: TransactionEnvelope =
        xdr::Transaction::new_tx(source_account, 100, 100, operation).into();
    let tx_env = tx_env.to_xdr_base64(Limits::none()).unwrap();

    let hash: xdr::Hash = sandbox
        .new_assert_cmd("tx")
        .arg("hash")
        .write_stdin(tx_env.as_bytes())
        .assert()
        .success()
        .stdout_as_str()
        .parse()
        .unwrap();

    let sign = tokio::task::spawn_blocking({
        let sandbox = Arc::clone(&sandbox);

        move || {
            sandbox
                .new_assert_cmd("tx")
                .arg("sign")
                .arg("--sign-with-ledger")
                .arg("--hd-path")
                .arg(hd_path.to_string())
                .write_stdin(tx_env.as_bytes())
                .env("SPECULOS_PORT", host_port.to_string())
                .env("RUST_LOGS", "trace")
                .assert()
                .success()
                .stdout_as_str()
        }
    });

    let approve = tokio::task::spawn(approve_tx_hash_signature(
        ui_host_port,
        ledger_device_model.to_string(),
    ));

    let response = sign.await.unwrap();
    approve.await.unwrap();
    let txn_env =
        xdr::TransactionEnvelope::from_xdr_base64(&response, xdr::Limits::none()).unwrap();
    let xdr::TransactionEnvelope::Tx(tx_env) = txn_env else {
        panic!("expected Tx")
    };
    let signatures = tx_env.signatures.to_vec();
    let signature = signatures[0].signature.to_vec();
    verifying_key
        .verify_strict(
            &hash.0,
            &ed25519_dalek::Signature::from_slice(&signature).unwrap(),
        )
        .unwrap();
}

// Mirrors `invoke_auth_with_non_source_identity` from the integration tests:
// invoke a contract whose `auth(addr, world)` calls `addr.require_auth()`,
// where the auth identity (`testone`) is a Ledger-backed alias and the
// transaction source (`test`) is a regular keypair. Exercises the Soroban
// auth-entry signing path through the Ledger device.
#[test_case("nanos", 0; "when the device is NanoS")]
#[test_case("nanox", 1; "when the device is NanoX")]
#[test_case("nanosp", 2; "when the device is NanoS Plus")]
#[tokio::test]
async fn invoke_auth_with_ledger_identity(ledger_device_model: &str, hd_path: u32) {
    let sandbox = Arc::new(TestEnv::new());
    let container = TestEnv::speculos_container(ledger_device_model).await;
    let host_port = container.get_host_port_ipv4(9998).await.unwrap();
    let ui_host_port = container.get_host_port_ipv4(5000).await.unwrap();

    sandbox
        .new_assert_cmd("keys")
        .arg("fund")
        .arg("test")
        .assert()
        .success();

    sandbox
        .new_assert_cmd("keys")
        .arg("add")
        .arg("testone")
        .arg("--ledger")
        .arg("--hd-path")
        .arg(hd_path.to_string())
        .env("SPECULOS_PORT", host_port.to_string())
        .assert()
        .success();

    let addr = sandbox
        .new_assert_cmd("keys")
        .arg("address")
        .arg("testone")
        .assert()
        .success()
        .stdout_as_str();

    let id = sandbox
        .new_assert_cmd("contract")
        .arg("deploy")
        .arg("--source")
        .arg("test")
        .arg("--wasm")
        .arg(HELLO_WORLD.path())
        .arg("--ignore-checks")
        .assert()
        .success()
        .stdout_as_str();

    let invoke = tokio::task::spawn_blocking({
        let sandbox = Arc::clone(&sandbox);
        let id = id.clone();
        let addr = addr.clone();
        move || {
            let stdout = sandbox
                .new_assert_cmd("contract")
                .arg("invoke")
                .arg("--source")
                .arg("test")
                .arg("--id")
                .arg(&id)
                .arg("--")
                .arg("auth")
                .arg("--addr")
                .arg("testone")
                .arg("--world=world")
                .env("SPECULOS_PORT", host_port.to_string())
                .assert()
                .success()
                .stdout_as_str();
            assert_eq!(stdout, format!("\"{addr}\""));
        }
    });

    let approve = tokio::task::spawn(approve_tx_hash_signature(
        ui_host_port,
        ledger_device_model.to_string(),
    ));

    invoke.await.unwrap();
    approve.await.unwrap();
}
