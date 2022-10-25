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

// e2e tests are ignore by default
#[test]
#[ignore]
fn e2e_deploy_and_invoke_contract_against_rpc_server() {
    // This test assumes a fresh standalone network rpc server on port 8000

    const WASM: &str = "target/wasm32-unknown-unknown/release/test_hello_world.wasm";
    assert!(
        Path::new(WASM).is_file(),
        "file {WASM:?} missing, run 'make test-wasms' to generate .wasm files before running this test"
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
