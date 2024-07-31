use soroban_test::{AssertExt, TestEnv};

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
    // dont overwrite if identity already exists
    let output = sandbox
        .new_assert_cmd("keys")
        .arg("generate")
        .arg("test")
        .assert()
        .stderr_as_str();
    assert!(output.contains("The identity test already exists!"));
    assert!(!output.contains("Generated new key"));
    // do overwrite if the identity exists but different seed passed
    let output = sandbox
        .new_assert_cmd("keys")
        .arg("generate")
        .arg("test")
        .arg("--seed")
        .arg("aaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        .assert()
        .stderr_as_str();
    assert!(output.contains("An identity with the name test already exists but has a different secret. Overwriting..."));
    assert!(output.contains("Generated new key"));
}
