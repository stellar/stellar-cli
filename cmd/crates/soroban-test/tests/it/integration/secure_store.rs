use predicates::prelude::predicate;
use soroban_cli::tx::ONE_XLM;
use soroban_test::{AssertExt, TestEnv};

fn secure_store_key(sandbox: &TestEnv, name: &str) -> String {
    sandbox
        .new_assert_cmd("keys")
        .args(["generate", "--fund", "--secure-store", name])
        .assert()
        .success()
        .stdout_as_str();

    sandbox
        .new_assert_cmd("keys")
        .args(["address", name])
        .assert()
        .success()
        .stdout_as_str()
}

// test that we can create a create-account tx and sign it with a secure-store key
#[tokio::test]
async fn create_account() {
    let sandbox = &TestEnv::new();
    let secure_store_address = secure_store_key(sandbox, "secure-store");

    sandbox
        .new_assert_cmd("keys")
        .args(["generate", "--no-fund", "new"])
        .assert()
        .success();
    let new_address = sandbox
        .new_assert_cmd("keys")
        .args(["address", "new"])
        .assert()
        .success()
        .stdout_as_str();

    let client = sandbox.network.rpc_client().unwrap();
    let secure_account = client.get_account(&secure_store_address).await.unwrap();

    let starting_balance = ONE_XLM * 100;
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "create-account",
            "--destination",
            new_address.as_str(),
            "--starting-balance",
            starting_balance.to_string().as_str(),
            "--source",
            "secure-store",
        ])
        .assert()
        .success()
        .stdout_as_str();

    let secure_account_after = client.get_account(&secure_store_address).await.unwrap();
    assert!(secure_account_after.balance < secure_account.balance);

    let new_account = client.get_account(&new_address).await.unwrap();
    assert_eq!(new_account.balance, starting_balance);
}

#[tokio::test]
async fn get_secret_key() {
    let sandbox = &TestEnv::new();
    sandbox
        .new_assert_cmd("keys")
        .args(["generate", "secret-key-test", "--secure-store"])
        .assert()
        .success();
    sandbox
        .new_assert_cmd("keys")
        .arg("secret")
        .arg("secret-key-test")
        .assert()
        .stderr(predicate::str::contains("does not reveal secret key"))
        .failure();
}

#[tokio::test]
async fn public_key_with_secure_store() {
    let sandbox = &TestEnv::new();
    sandbox
        .new_assert_cmd("keys")
        .args(["generate", "public-key-test", "--secure-store"])
        .assert()
        .success();
    sandbox
        .new_assert_cmd("keys")
        .arg("public-key")
        .arg("public-key-test")
        .assert()
        .stdout(predicate::str::contains("G"))
        .success();
}
