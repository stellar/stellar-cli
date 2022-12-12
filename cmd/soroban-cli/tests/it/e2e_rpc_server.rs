use assert_cmd::Command;
use std::str;

use crate::util::{arg_file, test_wasm, AssertUtil, SorobanCommand, Standalone};

// e2e tests are ignore by default
#[test]
#[ignore]
fn e2e_deploy_and_invoke_contract_against_rpc_server() {
    // This test assumes a fresh standalone network rpc server on port 8000
    let id = Standalone::new_cmd("deploy")
        .arg("--wasm")
        .arg(test_wasm("test_hello_world"))
        .arg("--salt=0")
        .assert()
        .stderr("success\nsuccess\n")
        .success()
        .output_line();

    test_hello_world(&id, |cmd| cmd.arg("--arg=world"));
    test_hello_world(&id, |cmd| cmd.arg("--arg-file").arg(arg_file("world")));
}

fn test_hello_world<F>(id: &str, f: F)
where
    F: FnOnce(&mut Command) -> &mut Command,
{
    f(Standalone::new_cmd("invoke")
        .args(["--id", id])
        .arg("--fn=hello"))
    .assert()
    .stdout("[\"Hello\",\"world\"]\n")
    .stderr("success\n")
    .success();
}

// e2e tests are ignore by default
#[test]
#[ignore]
fn e2e_install_deploy_and_invoke_contract_against_rpc_server() {
    // This test assumes a fresh standalone network rpc server on port 8000
    Standalone::new_cmd("install")
        .arg("--wasm")
        .arg(test_wasm("test_hello_world"))
        .assert()
        .stdout("86270dcca8dd4e7131c89dcc61223f096d7a1fa4a1d90c39dd6542b562369ecc\n")
        .stderr("success\n")
        .success();

    Standalone::new_cmd("deploy")
        .arg("--wasm-hash=86270dcca8dd4e7131c89dcc61223f096d7a1fa4a1d90c39dd6542b562369ecc")
        .arg("--salt=0")
        .assert()
        .stdout("b392cd0044315873f32307bfd535a9cbbb0402a57133ff7283afcae66be8174b\n")
        .stderr("success\n")
        .success();

    Standalone::new_cmd("invoke")
        .arg("--id=b392cd0044315873f32307bfd535a9cbbb0402a57133ff7283afcae66be8174b")
        .arg("--fn=hello")
        .arg("--arg=world")
        .assert()
        .stdout("[\"Hello\",\"world\"]\n")
        .stderr("success\n")
        .success();
}

#[test]
#[ignore]
fn create_and_invoke_token_contract_against_rpc_server() {
    // This test assumes a fresh standalone network rpc server on port 8000

    Standalone::new_cmd("token")
        .args([
            "create",
            "--name=Stellar Lumens",
            "--symbol=XLM",
            "--salt=1",
        ])
        .assert()
        .stdout("1bd2a2473623e73904d35a334476d1fe3cd192811bd823b7815fd9ce57c82232\n")
        .stderr("success\nsuccess\n")
        .success();

    Standalone::new_cmd("invoke")
        .args([
            "--id=1bd2a2473623e73904d35a334476d1fe3cd192811bd823b7815fd9ce57c82232",
            "--fn=symbol",
        ])
        .assert()
        .stdout("[88,76,77]\n")
        .stderr("success\n")
        .success();
}
