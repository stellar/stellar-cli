use crate::util::{output_line, temp_ledger_file, test_wasm, Sandbox, SorobanCommand};

#[test]
fn source_account_exists() {
    Sandbox::new_cmd("invoke")
        .arg("--ledger-file")
        .arg(temp_ledger_file())
        .arg("--id=1")
        .arg("--wasm")
        .arg(test_wasm("test_invoker_account_exists"))
        .arg("--fn=invkexists")
        .assert()
        .success()
        .stdout("true\n");
}

#[test]
fn install_wasm_then_deploy_contract() {
    let ledger = temp_ledger_file();
    let id = output_line(
        &Sandbox::new_cmd("install")
            .arg("--ledger-file")
            .arg(&ledger)
            .arg("--wasm")
            .arg(test_wasm("test_hello_world"))
            .assert()
            .success(),
    );

    Sandbox::new_cmd("deploy")
        .arg("--ledger-file")
        .arg(&ledger)
        .arg("--wasm-hash")
        .arg(&id)
        .arg("--id=1")
        .assert()
        .success()
        .stdout("0000000000000000000000000000000000000000000000000000000000000001\n");
}

#[test]
fn deploy_contract_with_wasm_file() {
    Sandbox::new_cmd("deploy")
        .arg("--ledger-file")
        .arg(temp_ledger_file())
        .arg("--wasm")
        .arg(test_wasm("test_hello_world"))
        .arg("--id=1")
        .assert()
        .success()
        .stdout("0000000000000000000000000000000000000000000000000000000000000001\n");
}

#[test]
fn invoke_hello_world_with_deploy_first() {
    // This test assumes a fresh standalone network rpc server on port 8000
    let ledger = temp_ledger_file();
    let res = Sandbox::new_cmd("deploy")
        .arg("--wasm")
        .arg(test_wasm("test_hello_world"))
        .arg("--ledger-file")
        .arg(&ledger)
        .assert()
        .success();
    let stdout = String::from_utf8(res.get_output().stdout.clone()).unwrap();
    let id = stdout.trim_end();

    Sandbox::new_cmd("invoke")
        .arg("--ledger-file")
        .arg(&ledger)
        .arg("--id")
        .arg(id)
        .arg("--fn=hello")
        .arg("--")
        .arg("--world=world")
        .assert()
        .stdout("[\"Hello\",\"world\"]\n")
        .success();
}

#[test]
fn invoke_hello_world() {
    // This test assumes a fresh standalone network rpc server on port 8000
    let ledger = temp_ledger_file();
    Sandbox::new_cmd("invoke")
        .arg("--id=1")
        .arg("--wasm")
        .arg(test_wasm("test_hello_world"))
        .arg("--ledger-file")
        .arg(&ledger)
        .arg("--fn=hello")
        .arg("--")
        .arg("--world=world")
        .assert()
        .stdout("[\"Hello\",\"world\"]\n")
        .success();
}
