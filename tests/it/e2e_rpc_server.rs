use assert_cmd::Command;

use crate::util::{test_wasm, SorobanCommand, Standalone};

// e2e tests are ignore by default
#[test]
#[ignore]
fn e2e_deploy_and_invoke_contract_against_rpc_server() {
    // This test assumes a fresh standalone network rpc server on port 8000
    Standalone::new_cmd()
        .arg("deploy")
        .arg("--wasm")
        .arg(test_wasm("test_hello_world"))
        .arg("--salt=0")
        .assert()
        .stdout("1f3eb7b8dc051d6aa46db5454588a142c671a0cdcdb36a2f754d9675a64bf613\n")
        .stderr("success\n")
        .success();

    test_hello_world(move |cmd| cmd.arg("--arg=world"));
    test_hello_world(move |cmd| cmd.arg("--arg-file=./cmd/soroban-cli/tests/fixtures/args/world"))
}

fn test_hello_world<F>(f: F)
where
    F: FnOnce(&mut Command) -> &mut Command,
{
    f(Standalone::new_cmd()
        .arg("invoke")
        .arg("--id=1f3eb7b8dc051d6aa46db5454588a142c671a0cdcdb36a2f754d9675a64bf613")
        .arg("--fn=hello"))
    .assert()
    .stdout("[\"Hello\",\"world\"]\n")
    .stderr("success\n")
    .success();
}

#[test]
#[ignore]
fn create_and_invoke_token_contract_against_rpc_server() {
    // This test assumes a fresh standalone network rpc server on port 8000

    Standalone::new_cmd()
        .args([
            "token",
            "create",
            "--name=Stellar Lumens",
            "--symbol=XLM",
            "--salt=1",
        ])
        .assert()
        .stdout("8af3f0c5c2c4b5a3c6ac67b390f84d9db843b48827376f42e5bad215c42588f7\n")
        .stderr("success\nsuccess\n")
        .success();

    Standalone::new_cmd()
        .args([
            "invoke",
            "--id=8af3f0c5c2c4b5a3c6ac67b390f84d9db843b48827376f42e5bad215c42588f7",
            "--fn=symbol",
        ])
        .assert()
        .stdout("[88,76,77]\n")
        .stderr("success\n")
        .success();
}
