use crate::util::{test_wasm, AssertExt, SorobanCommand, Standalone};

// e2e tests are ignore by default
#[test]
#[ignore]
fn e2e_deploy_and_invoke_contract_against_rpc_server() {
    // This test assumes a fresh standalone network rpc server on port 8000
    let id = Standalone::new_cmd("deploy")
        .arg("--wasm")
        .arg(test_wasm("test_hello_world"))
        .assert()
        .stderr("success\nsuccess\n")
        .success()
        .output_line();

    Standalone::new_cmd("invoke")
        .args(["--id", &id])
        .arg("--fn=hello")
        .arg("--")
        .args(["--world", "world"])
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
        .stdout("ea2b54f1eee052444b46603c1ffa8cabebb224de0bb83182f65e02c133fab035\n")
        .stderr("success\n")
        .success();

    Standalone::new_cmd("deploy")
        .arg("--wasm-hash=ea2b54f1eee052444b46603c1ffa8cabebb224de0bb83182f65e02c133fab035")
        .arg("--salt=0")
        .assert()
        .stdout("b392cd0044315873f32307bfd535a9cbbb0402a57133ff7283afcae66be8174b\n")
        .stderr("success\n")
        .success();

    Standalone::new_cmd("invoke")
        .arg("--id=b392cd0044315873f32307bfd535a9cbbb0402a57133ff7283afcae66be8174b")
        .arg("--fn=hello")
        .arg("--")
        .arg("--world=world")
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
        .stdout("\"584c4d\"\n")
        .stderr("success\n")
        .success();
}
