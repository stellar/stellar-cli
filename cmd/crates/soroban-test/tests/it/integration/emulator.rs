use stellar_ledger::{Blob, Error};

use soroban_test::{AssertExt, TestEnv, LOCAL_NETWORK_PASSPHRASE};
use std::sync::Arc;

use soroban_cli::xdr::{
    self, Memo, MuxedAccount, Operation, OperationBody, PaymentOp, Preconditions, SequenceNumber,
    Transaction, TransactionExt, Uint256,
};

use stellar_ledger::emulator_test_support::*;

use test_case::test_case;

use crate::integration::util::{deploy_contract, DeployKind, HELLO_WORLD};

// #[test_case("nanos"; "when the device is NanoS")]
#[test_case("nanox"; "when the device is NanoX")]
// #[test_case("nanosp"; "when the device is NanoS Plus")]
#[tokio::test]
async fn test_get_public_key(ledger_device_model: &str) {
    let sandbox = Arc::new(TestEnv::new());
    let container = TestEnv::speculos_container(ledger_device_model).await;
    let host_port = container.get_host_port_ipv4(9998).await.unwrap();
    let ui_host_port = container.get_host_port_ipv4(5000).await.unwrap();

    let ledger = ledger(host_port).await;

    let key = ledger.get_public_key(&0.into()).await.unwrap();
    let account = &key.to_string();
    sandbox.fund_account(account);
    sandbox
        .new_assert_cmd("contract")
        .arg("install")
        .args([
            "--wasm",
            HELLO_WORLD.path().as_os_str().to_str().unwrap(),
            "--source",
            account,
        ])
        .assert()
        .success();

    let tx_simulated =
        deploy_contract(&sandbox, HELLO_WORLD, DeployKind::SimOnly, Some(account)).await;
    dbg!("{tx_simulated}");
    let key = ledger.get_public_key(&0.into()).await.unwrap();
    println!("{key}");
    let sign = tokio::task::spawn_blocking({
        let sandbox = Arc::clone(&sandbox);

        move || {
            sandbox
                .new_assert_cmd("tx")
                .arg("sign")
                .arg("--sign-with-ledger")
                .write_stdin(tx_simulated.as_bytes())
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

    dbg!("{tx_signed}");

    sandbox
        .clone()
        .new_assert_cmd("tx")
        .arg("send")
        .write_stdin(response.as_bytes())
        .assert()
        .success()
        .stdout(predicates::str::contains("SUCCESS"));
}
