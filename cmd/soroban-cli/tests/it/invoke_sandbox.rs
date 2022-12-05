use crate::util::{temp_ledger_file, test_wasm, Sandbox, SorobanCommand};

#[test]
fn invoke_token() {
    let ledger = temp_ledger_file();
    Sandbox::new_cmd()
        .arg("token")
        .arg("create")
        .arg("--ledger-file")
        .arg(&ledger)
        .arg("--name=tok")
        .arg("--symbol=tok")
        .assert()
        .success()
        .stdout("7794c4a02357bd9063499148e709bde44aa9e643d3fa20fde202f6e84a671e1b\n");

    Sandbox::new_cmd()
        .arg("invoke")
        .arg("--ledger-file")
        .arg(ledger)
        .arg("--id=7794c4a02357bd9063499148e709bde44aa9e643d3fa20fde202f6e84a671e1b")
        .arg("--fn=decimals")
        .assert()
        .success()
        .stdout("7\n");
}

#[test]
fn source_account_exists() {
    Sandbox::new_cmd()
        .arg("invoke")
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
