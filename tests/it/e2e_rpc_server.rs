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

    Standalone::new_cmd()
        .arg("invoke")
        .arg("--id=1f3eb7b8dc051d6aa46db5454588a142c671a0cdcdb36a2f754d9675a64bf613")
        .arg("--fn=hello")
        .arg("--arg=world")
        .assert()
        .stdout("[\"Hello\",\"world\"]\n")
        .stderr("success\n")
        .success();
}
