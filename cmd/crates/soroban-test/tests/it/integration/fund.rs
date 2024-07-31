use soroban_test::TestEnv;

#[tokio::test]
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
        // Don't expect error if friendbot indicated that the account is
        // already fully funded to the starting balance, because the
        // user's goal is to get funded, and the account is funded
        // so it is success much the same.
        .success();
}
