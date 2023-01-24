use crate::util::{temp_ledger_file, Sandbox, SorobanCommand, HELLO_WORLD, INVOKER_ACCOUNT_EXISTS};

#[test]
fn source_account_exists() {
    Sandbox::new_cmd("contract")
        .arg("invoke")
        .arg("--ledger-file")
        .arg(temp_ledger_file())
        .arg("--id=1")
        .arg("--wasm")
        .arg(INVOKER_ACCOUNT_EXISTS.path())
        .arg("--fn=invkexists")
        .assert()
        .success()
        .stdout("true\n");
}

#[test]
fn install_wasm_then_deploy_contract() {
    let ledger = temp_ledger_file();
    let hash = HELLO_WORLD.hash();
    Sandbox::new_cmd("contract")
        .arg("install")
        .arg("--ledger-file")
        .arg(&ledger)
        .arg("--wasm")
        .arg(HELLO_WORLD.path())
        .assert()
        .success()
        .stdout(format!("{hash}\n"));

    Sandbox::new_cmd("contract")
        .arg("deploy")
        .arg("--ledger-file")
        .arg(&ledger)
        .arg("--wasm-hash")
        .arg(&format!("{hash}"))
        .arg("--id=1")
        .assert()
        .success()
        .stdout("0000000000000000000000000000000000000000000000000000000000000001\n");
}

#[test]
fn deploy_contract_with_wasm_file() {
    Sandbox::new_cmd("contract")
        .arg("deploy")
        .arg("--ledger-file")
        .arg(temp_ledger_file())
        .arg("--wasm")
        .arg(HELLO_WORLD.path())
        .arg("--id=1")
        .assert()
        .success()
        .stdout("0000000000000000000000000000000000000000000000000000000000000001\n");
}

#[test]
fn invoke_hello_world_with_deploy_first() {
    // This test assumes a fresh standalone network rpc server on port 8000
    let ledger = temp_ledger_file();
    let res = Sandbox::new_cmd("contract")
        .arg("deploy")
        .arg("--wasm")
        .arg(HELLO_WORLD.path())
        .arg("--ledger-file")
        .arg(&ledger)
        .assert()
        .success();
    let stdout = String::from_utf8(res.get_output().stdout.clone()).unwrap();
    let id = stdout.trim_end();

    Sandbox::new_cmd("contract")
        .arg("invoke")
        .arg("--identity")
        .arg("test_id")
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
    Sandbox::new_cmd("contract")
        .arg("invoke")
        .arg("--id=1")
        .arg("--wasm")
        .arg(HELLO_WORLD.path())
        .arg("--ledger-file")
        .arg(&ledger)
        .arg("--fn=hello")
        .arg("--")
        .arg("--world=world")
        .assert()
        .stdout("[\"Hello\",\"world\"]\n")
        .success();
}
