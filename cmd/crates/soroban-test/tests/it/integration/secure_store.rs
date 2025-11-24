use predicates::prelude::predicate;
use soroban_cli::tx::ONE_XLM;
use soroban_test::{AssertExt, TestEnv};

// All secure store tests are run within one test to avoid issues with multiple
// tests trying to access the dbus at the same time which can lead to intermittent failures.
#[tokio::test]
async fn secure_store_key_management() {
    let sandbox = &TestEnv::new();

    let secure_key_name = "secure-store-test";

    // generate a new secret key in secure store
    sandbox
        .new_assert_cmd("keys")
        .args(["generate", secure_key_name, "--secure-store", "--fund"])
        .assert()
        .success();

    // validate that we cannot get the secret key back
    sandbox
        .new_assert_cmd("keys")
        .arg("secret")
        .arg(secure_key_name)
        .assert()
        .stderr(predicate::str::contains("does not reveal secret key"))
        .failure();

    // validate that we can get the public key
    let secure_store_address = sandbox
        .new_assert_cmd("keys")
        .args(["address", secure_key_name])
        .assert()
        .success()
        .stdout_as_str();
    assert!(secure_store_address.starts_with('G'));

    // use the secure store key to fund a new account
    let new_key_name = "new";
    sandbox
        .new_assert_cmd("keys")
        .args(["generate", new_key_name])
        .assert()
        .success();
    let new_address = sandbox
        .new_assert_cmd("keys")
        .args(["address", new_key_name])
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
            secure_key_name,
        ])
        .assert()
        .success()
        .stdout_as_str();

    let secure_account_after = client.get_account(&secure_store_address).await.unwrap();
    assert!(secure_account_after.balance < secure_account.balance);

    let new_account = client.get_account(&new_address).await.unwrap();
    assert_eq!(new_account.balance, starting_balance);
}
