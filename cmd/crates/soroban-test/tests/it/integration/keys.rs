use predicates::prelude::predicate;
use soroban_test::AssertExt;
use soroban_test::TestEnv;

fn pubkey_for_identity(sandbox: &TestEnv, name: &str) -> String {
    sandbox
        .new_assert_cmd("keys")
        .arg("address")
        .arg(name)
        .assert()
        .stdout_as_str()
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn fund() {
    let sandbox = &TestEnv::new();
    sandbox
        .new_assert_cmd("keys")
        .arg("generate")
        .arg("test2")
        .assert()
        .success();
    sandbox
        .new_assert_cmd("keys")
        .arg("fund")
        .arg("test2")
        .assert()
        // Don't expect error if friendbot indicated that the account is
        // already fully funded to the starting balance, because the
        // user's goal is to get funded, and the account is funded
        // so it is success much the same.
        .success();
}

#[tokio::test]
async fn secret() {
    let sandbox = &TestEnv::new();
    sandbox
        .new_assert_cmd("keys")
        .arg("generate")
        .arg("test2")
        .assert()
        .success();
    sandbox
        .new_assert_cmd("keys")
        .arg("secret")
        .arg("test2")
        .assert()
        .success();
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn overwrite_identity() {
    let sandbox = &TestEnv::new();
    sandbox
        .new_assert_cmd("keys")
        .arg("generate")
        .arg("test2")
        .assert()
        .success();

    let initial_pubkey = sandbox
        .new_assert_cmd("keys")
        .arg("address")
        .arg("test2")
        .assert()
        .stdout_as_str();

    sandbox
        .new_assert_cmd("keys")
        .arg("generate")
        .arg("test2")
        .assert()
        .stderr(predicate::str::contains(
            "error: An identity with the name 'test2' already exists",
        ));

    assert_eq!(initial_pubkey, pubkey_for_identity(sandbox, "test2"));

    sandbox
        .new_assert_cmd("keys")
        .arg("generate")
        .arg("test2")
        .arg("--overwrite")
        .assert()
        .stderr(predicate::str::contains("Overwriting identity 'test2'"))
        .success();

    assert_ne!(initial_pubkey, pubkey_for_identity(sandbox, "test2"));
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn overwrite_identity_with_add() {
    let sandbox = &TestEnv::new();
    sandbox
        .new_assert_cmd("keys")
        .arg("generate")
        .arg("test3")
        .assert()
        .success();

    let initial_pubkey = sandbox
        .new_assert_cmd("keys")
        .arg("address")
        .arg("test3")
        .assert()
        .stdout_as_str();

    // Try to add a key with the same name, should fail
    sandbox
        .new_assert_cmd("keys")
        .arg("add")
        .arg("test3")
        .arg("--public-key")
        .arg("GAKSH6AD2IPJQELTHIOWDAPYX74YELUOWJLI2L4RIPIPZH6YQIFNUSDC")
        .assert()
        .stderr(predicate::str::contains(
            "error: An identity with the name 'test3' already exists",
        ));

    // Verify the key wasn't changed
    assert_eq!(initial_pubkey, pubkey_for_identity(sandbox, "test3"));

    // Try again with --overwrite flag, should succeed
    sandbox
        .new_assert_cmd("keys")
        .arg("add")
        .arg("test3")
        .arg("--public-key")
        .arg("GAKSH6AD2IPJQELTHIOWDAPYX74YELUOWJLI2L4RIPIPZH6YQIFNUSDC")
        .arg("--overwrite")
        .assert()
        .stderr(predicate::str::contains("Overwriting identity 'test3'"))
        .success();

    // Verify the key was changed
    assert_ne!(initial_pubkey, pubkey_for_identity(sandbox, "test3"));
    assert_eq!(
        "GAKSH6AD2IPJQELTHIOWDAPYX74YELUOWJLI2L4RIPIPZH6YQIFNUSDC",
        pubkey_for_identity(sandbox, "test3").trim()
    );
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn set_default_identity() {
    let sandbox = &TestEnv::new();
    sandbox
        .new_assert_cmd("keys")
        .arg("generate")
        .arg("test4")
        .assert()
        .success();

    sandbox
        .new_assert_cmd("keys")
        .arg("use")
        .arg("test4")
        .assert()
        .stderr(predicate::str::contains(
            "The default source account is set to `test4`",
        ))
        .success();
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn clear_default_identity() {
    let sandbox = &TestEnv::new();
    sandbox
        .new_assert_cmd("keys")
        .arg("generate")
        .arg("test5")
        .assert()
        .success();

    sandbox
        .new_assert_cmd("keys")
        .arg("use")
        .arg("test5")
        .assert()
        .stderr(predicate::str::contains(
            "The default source account is set to `test5`",
        ))
        .success();

    sandbox
        .new_assert_cmd("keys")
        .arg("use")
        .arg("--clear")
        .assert()
        .stderr(predicate::str::contains(
            "The default source account has been cleared",
        ))
        .success();
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn validate_use_without_name() {
    let sandbox = &TestEnv::new();

    sandbox
        .new_assert_cmd("keys")
        .arg("use")
        .assert()
        .stderr(predicate::str::contains(
            "error: Identify name is required unless --clear is specified",
        ))
        .failure();
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn validate_use_with_both_name_and_clear() {
    let sandbox = &TestEnv::new();

    sandbox
        .new_assert_cmd("keys")
        .arg("use")
        .arg("test5")
        .arg("--clear")
        .assert()
        .stderr(predicate::str::contains(
            "error: Identify name cannot be used with --clear",
        ))
        .failure();
}
