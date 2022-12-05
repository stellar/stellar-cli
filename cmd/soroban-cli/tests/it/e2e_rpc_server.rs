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
        .stdout("b392cd0044315873f32307bfd535a9cbbb0402a57133ff7283afcae66be8174b\n")
        .stderr("success\nsuccess\n")
        .success();

    Standalone::new_cmd()
        .arg("invoke")
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

    Standalone::new_cmd()
        .args([
            "token",
            "create",
            "--name=Stellar Lumens",
            "--symbol=XLM",
            "--salt=1",
        ])
        .assert()
        .stdout("1bd2a2473623e73904d35a334476d1fe3cd192811bd823b7815fd9ce57c82232\n")
        .stderr("success\nsuccess\n")
        .success();

    Standalone::new_cmd()
        .args([
            "invoke",
            "--id=1bd2a2473623e73904d35a334476d1fe3cd192811bd823b7815fd9ce57c82232",
            "--fn=symbol",
        ])
        .assert()
        .stdout("[88,76,77]\n")
        .stderr("success\n")
        .success();
}
