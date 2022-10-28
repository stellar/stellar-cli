use crate::util::{temp_ledger_file, test_wasm, Sandbox, SorobanCommand};

#[test]
fn invoke_token() {
    let ledger = temp_ledger_file();
    Sandbox::new()
        .arg("token")
        .arg("create")
        .arg("--ledger-file")
        .arg(&ledger)
        .arg("--name=tok")
        .arg("--symbol=tok")
        .assert()
        .success()
        .stdout("d55b5a3a5793539545f957f7da783f7b19159369ccdb19c53dbd117ebfc08842\n");

    Sandbox::new()
        .arg("invoke")
        .arg("--ledger-file")
        .arg(ledger)
        .arg("--id=d55b5a3a5793539545f957f7da783f7b19159369ccdb19c53dbd117ebfc08842")
        .arg("--fn=decimals")
        .assert()
        .success()
        .stdout("7\n");
}

#[test]
fn source_account_exists() {
    Sandbox::new()
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
