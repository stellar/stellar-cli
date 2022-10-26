#[cfg(test)]
use crate::util::Soroban;

#[test]
fn deploy_and_invoke_contract_against_rpc_server() -> anyhow::Result<()> {
    // This test assumes a fresh standalone network rpc server on port 8000

    Soroban::new_standalone()?
        .arg("deploy")
        .wasm("tests/fixtures/soroban_hello_world_contract.wasm")?
        .arg("--salt=0")
        .assert()
        .stdout("1f3eb7b8dc051d6aa46db5454588a142c671a0cdcdb36a2f754d9675a64bf613\n")
        .stderr("success\n")
        .success();

    Soroban::new_standalone()?
        .arg("invoke")
        .contract_id("1f3eb7b8dc051d6aa46db5454588a142c671a0cdcdb36a2f754d9675a64bf613")
        ._fn("hello")
        ._arg("world")
        .assert()
        .stdout("[\"Hello\",\"world\"]\n")
        .stderr("success\n")
        .success();
    Ok(())
}
