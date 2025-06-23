use soroban_test::TestEnv;

#[tokio::test]
async fn ledger_latest() {
    let sandbox = &TestEnv::new();
    sandbox
        .new_assert_cmd("ledger")
        .arg("latest")
        .arg("--network")
        .arg("testnet")
        .assert()
        .success()
        .stdout(predicates::str::contains("Sequence:"))
        .stdout(predicates::str::contains("Protocol Version:"))
        .stdout(predicates::str::contains("Hash:"));
}
