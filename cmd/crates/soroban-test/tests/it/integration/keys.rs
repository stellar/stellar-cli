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
async fn secret_with_secure_store_key() {
    let sandbox = &TestEnv::new();
    sandbox
        .new_assert_cmd("keys")
        .args(["generate", "test2", "--secure-store"])
        .assert()
        .success();
    sandbox
        .new_assert_cmd("keys")
        .arg("secret")
        .arg("test2")
        .assert()
        .stderr(predicate::str::contains("does not reveal secret key"))
        .failure();
}

#[tokio::test]
async fn public_key_with_secure_store_key() {
    let sandbox = &TestEnv::new();
    sandbox
        .new_assert_cmd("keys")
        .args(["generate", "test2", "--secure-store"])
        .assert()
        .success();
    sandbox
        .new_assert_cmd("keys")
        .arg("public-key")
        .arg("test2")
        .assert()
        .stdout(predicate::str::contains("G"))
        .success();
}
