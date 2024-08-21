use soroban_test::{AssertExt, TestEnv};

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn fund() {
    let sandbox = &TestEnv::new();
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
    // dont overwrite if identity already exists
    let output = sandbox
        .new_assert_cmd("keys")
        .arg("generate")
        .arg("test")
        .assert()
        .stderr_as_str();
    assert!(output.contains("The identity test already exists!"));
    assert!(!output.contains("Generated new key"));
    // do overwrite if forced
    let output = sandbox
        .new_assert_cmd("keys")
        .arg("generate")
        .arg("test")
        .arg("--overwrite")
        .assert()
        .stderr_as_str();
    assert!(output.contains("Overwriting existing identity 'test' as requested."));
    assert!(output.contains("Generated new key"));
}
