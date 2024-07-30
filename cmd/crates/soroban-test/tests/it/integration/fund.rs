use soroban_test::TestEnv;

#[allow(clippy::too_many_lines)]
async fn fund() {
    let sandbox = &TestEnv::new();
    sandbox
        .new_assert_cmd("keys")
        .arg("generate")
        .arg("test")
        .assert()
        .success();
    sandbox
        .new_assert_cmd("keys")
        .arg("fund")
        .arg("test")
        .assert()
        .stderr(predicates::str::contains("funding failed"));
}
