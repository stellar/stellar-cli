use crate::util::{test_wasm, SorobanCommand, Standalone};
use std::str;

// e2e tests are ignore by default
#[test]
#[ignore]
fn e2e_deploy_and_invoke_contract_against_rpc_server() {
    // This test assumes a fresh standalone network rpc server on port 8000

    let result = &Standalone::new_cmd()
        .arg("deploy")
        .arg("--wasm")
        .arg(test_wasm("test_hello_world"))
        .assert()
        .stderr("success\nsuccess\n")
        .success();

    let id = str::from_utf8(&result.get_output().stdout).unwrap().trim();

    Standalone::new_cmd()
        .arg("invoke")
        .arg("--id")
        .arg(id)
        .arg("--fn=hello")
        .arg("--arg=world")
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
    let install_result = Standalone::new_cmd()
        .arg("install")
        .arg("--wasm")
        .arg(test_wasm("test_hello_world"))
        .assert()
        .stderr("success\n")
        .success();

    let wasm_hash = str::from_utf8(&install_result.get_output().stdout)
        .unwrap()
        .trim();

    let deploy_result = &Standalone::new_cmd()
        .arg("deploy")
        .arg("--wasm-hash")
        .arg(wasm_hash)
        .assert()
        .stderr("success\n")
        .success();

    let id = str::from_utf8(&deploy_result.get_output().stdout)
        .unwrap()
        .trim();

    Standalone::new_cmd()
        .arg("invoke")
        .arg("--id")
        .arg(id)
        .arg("--fn=hello")
        .arg("--arg=world")
        .assert()
        .stdout("[\"Hello\",\"world\"]\n")
        .stderr("success\n")
        .success();
}
