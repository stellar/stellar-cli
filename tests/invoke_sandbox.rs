use assert_cmd::prelude::*;
use assert_fs::{prelude::*, TempDir};
use std::process::Command;

#[test]
fn invoke_token() -> Result<(), Box<dyn std::error::Error>> {
    let tmp = TempDir::new().unwrap();
    let ledger = tmp.child("ledger.json");

    let mut cmd = Command::cargo_bin("soroban")?;
    cmd.arg("token").arg("create");
    cmd.arg("--ledger-file").arg(ledger.as_os_str());
    cmd.arg("--name").arg("tok");
    cmd.arg("--symbol").arg("tok");
    cmd.assert()
        .success()
        .stdout("d55b5a3a5793539545f957f7da783f7b19159369ccdb19c53dbd117ebfc08842\n");

    let mut cmd = Command::cargo_bin("soroban")?;
    cmd.arg("invoke");
    cmd.arg("--ledger-file").arg(ledger.path());
    cmd.arg("--id")
        .arg("d55b5a3a5793539545f957f7da783f7b19159369ccdb19c53dbd117ebfc08842");
    cmd.arg("--fn").arg("decimals");
    cmd.assert().success().stdout("7\n");

    Ok(())
}

#[test]
fn source_account_exists() -> Result<(), Box<dyn std::error::Error>> {
    let tmp = TempDir::new().unwrap();
    let ledger = tmp.child("ledger.json");

    let mut cmd = Command::cargo_bin("soroban")?;
    cmd.arg("invoke");
    cmd.arg("--ledger-file").arg(ledger.as_os_str());
    cmd.arg("--id").arg("1");
    cmd.arg("--wasm")
        .arg("target/wasm32-unknown-unknown/test-wasm/test_invoker_account_exists.wasm");
    cmd.arg("--fn").arg("invokerexi");
    cmd.assert().success().stdout("true\n");

    Ok(())
}
