use std::path::Path;

use assert_cmd::Command;

fn get_base_cmd() -> Command {
    let mut cmd = Command::cargo_bin("soroban").unwrap();
    cmd.env("SOROBAN_RPC_URL", "http://localhost:8000/soroban/rpc")
        .env(
            "SOROBAN_SECRET_KEY",
            "SC5O7VZUXDJ6JBDSZ74DSERXL7W3Y5LTOAMRF7RQRL3TAGAPS7LUVG3L",
        )
        .env(
            "SOROBAN_NETWORK_PASSPHRASE",
            "Standalone Network ; February 2017",
        );
    cmd
}

// e2e tests are ignored by default
#[test]
#[ignore]
fn deploy_and_invoke_contract_against_rpc_server() {
    // This test assumes a fresh standalone network rpc server on port 8000

    const WASM: &str = "target/wasm32-unknown-unknown/test-wasms/test_hello_world.wasm";
    assert!(
        Path::new(WASM).is_file(),
        "file {WASM:?} missing, run 'make build-test-wasms' to generate .wasm files before running this test"
    );

    let mut deploy = get_base_cmd();
    let deploy = deploy.args(&["deploy", "--wasm", WASM, "--salt=0"]);

    deploy
        .assert()
        .stdout("1f3eb7b8dc051d6aa46db5454588a142c671a0cdcdb36a2f754d9675a64bf613\n")
        .stderr("success\n")
        .success();

    let mut invoke = get_base_cmd();
    let invoke = invoke.args(&[
        "invoke",
        "--id=1f3eb7b8dc051d6aa46db5454588a142c671a0cdcdb36a2f754d9675a64bf613",
        "--fn=hello",
        "--arg=world",
    ]);

    invoke
        .assert()
        .stdout("[\"Hello\",\"world\"]\n")
        .stderr("success\n")
        .success();
}

#[test]
#[ignore]
fn create_and_invoke_token_contract_against_rpc_server() {
    // This test assumes a fresh standalone network rpc server on port 8000

    let mut deploy = get_base_cmd();
    let create = deploy.args(&[
        "token",
        "create",
        "--name=Stellar Lumens",
        "--symbol=XLM",
        "--salt=1",
    ]);

    create
        .assert()
        .stdout("8af3f0c5c2c4b5a3c6ac67b390f84d9db843b48827376f42e5bad215c42588f7\n")
        .stderr("success\nsuccess\n")
        .success();

    let mut invoke = get_base_cmd();
    let invoke = invoke.args(&[
        "invoke",
        "--id=8af3f0c5c2c4b5a3c6ac67b390f84d9db843b48827376f42e5bad215c42588f7",
        "--fn=symbol",
    ]);

    invoke
        .assert()
        .stdout("[88,76,77]\n")
        .stderr("success\n")
        .success();
}
