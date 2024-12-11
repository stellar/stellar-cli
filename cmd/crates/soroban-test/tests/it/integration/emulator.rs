use stellar_ledger::Blob;

use soroban_test::{AssertExt, TestEnv};
use std::sync::Arc;

use stellar_ledger::emulator_test_support::*;

use crate::integration::util::{self, deploy_contract, DeployKind, HELLO_WORLD};

#[tokio::test]
async fn nanos() {
    let sandbox = Arc::new(TestEnv::new());
    test_signer(&sandbox, "nanos", 0).await;
    test_signer(&sandbox, "nanox", 1).await;
    test_signer(&sandbox, "nanosp", 2).await;
}

#[tokio::test]
async fn nanox() {
    let sandbox = Arc::new(TestEnv::new());
    test_signer(&sandbox, "nanox", 1).await;
}

#[tokio::test]
async fn nanosp() {
    let sandbox = Arc::new(TestEnv::new());
    test_signer(&sandbox, "nanosp", 2).await;
}

async fn test_signer(sandbox: &Arc<TestEnv>, ledger_device_model: &str, hd_path: u32) {
    let container = TestEnv::speculos_container(ledger_device_model).await;
    let host_port = container.get_host_port_ipv4(9998).await.unwrap();
    let ui_host_port = container.get_host_port_ipv4(5000).await.unwrap();

    let ledger = ledger(host_port).await;

    let key = ledger.get_public_key(&hd_path.into()).await.unwrap();
    let contract = match hd_path {
        0 => HELLO_WORLD,
        1 => util::CUSTOM_ACCOUNT,
        2 => util::CUSTOM_TYPES,
        _ => panic!("Invalid hd_path"),
    };
    let account = key.to_string();
    sandbox.fund_account(&account);

    sandbox
        .new_assert_cmd("contract")
        .arg("install")
        .args(["--wasm", contract.path().as_os_str().to_str().unwrap()])
        .assert()
        .success();

    let tx_simulated = deploy_contract(
        sandbox,
        contract,
        crate::integration::util::DeployOptions {
            kind: DeployKind::SimOnly,
            deployer: Some(account),
            ..Default::default()
        },
    )
    .await;
    let sign = tokio::task::spawn_blocking({
        let sandbox = Arc::clone(sandbox);

        move || {
            sandbox
                .new_assert_cmd("tx")
                .arg("sign")
                .arg("--sign-with-ledger")
                .arg("--hd-path")
                .arg(hd_path.to_string())
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

    sandbox
        .clone()
        .new_assert_cmd("tx")
        .arg("send")
        .write_stdin(response.as_bytes())
        .assert()
        .success()
        .stdout(predicates::str::contains("SUCCESS"));
}
