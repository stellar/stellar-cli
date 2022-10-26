use crate::util::{temp_ledger_file, wasm_file, Soroban};

#[test]
fn invoke_token() -> Result<(), Box<dyn std::error::Error>> {
    let ledger = temp_ledger_file();
    Soroban::new()?
        .arg("token")
        .arg("create")
        .ledger_file(&ledger)
        .arg("--name")
        .arg("tok")
        .arg("--symbol")
        .arg("tok")
        .assert()
        .success()
        .stdout("d55b5a3a5793539545f957f7da783f7b19159369ccdb19c53dbd117ebfc08842\n");

    Soroban::invoke()?
        .ledger_file(&ledger)
        .contract_id("d55b5a3a5793539545f957f7da783f7b19159369ccdb19c53dbd117ebfc08842")
        ._fn("decimals")
        .assert()
        .success()
        .stdout("7\n");
    Ok(())
}

#[test]
fn source_account_exists() -> Result<(), Box<dyn std::error::Error>> {
    Soroban::invoke()?
        .ledger_file(temp_ledger_file())
        .contract_id("1")
        .wasm(wasm_file("test_invoker_account_exists"))?
        ._fn("invkexists")
        .assert()
        .success()
        .stdout("true\n");
    Ok(())
}
