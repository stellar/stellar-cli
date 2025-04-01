use soroban_cli::{
    config::locator,
    tx::{builder, ONE_XLM},
    utils::contract_id_hash_from_asset,
    xdr::{self, ReadXdr, SequenceNumber},
};
use soroban_rpc::LedgerEntryResult;
use soroban_test::{AssertExt, TestEnv};
use predicates::prelude::predicate;
use crate::integration::{
    hello_world::invoke_hello_world,
    util::{deploy_contract, DeployOptions, HELLO_WORLD},
    tx::build_sim_sign_send 
};
use soroban_cli::signer::keyring::keyring_mock;

pub fn test_address(sandbox: &TestEnv) -> String {
    sandbox
        .new_assert_cmd("keys")
        .arg("address")
        .arg("test")
        .assert()
        .success()
        .stdout_as_str()
}

fn new_account(sandbox: &TestEnv, name: &str) -> String {
    sandbox.generate_account(name, None).assert().success();
    sandbox
        .new_assert_cmd("keys")
        .args(["address", name])
        .assert()
        .success()
        .stdout_as_str()
}

fn gen_account_no_fund(sandbox: &TestEnv, name: &str) -> String {
    sandbox
        .new_assert_cmd("keys")
        .args(["generate", "--no-fund", name])
        .assert()
        .success();
    sandbox
        .new_assert_cmd("keys")
        .args(["address", name])
        .assert()
        .success()
        .stdout_as_str()
}

// todo: move these functions to utils for reusability
// returns test and test1 addresses
fn setup_accounts(sandbox: &TestEnv) -> (String, String) {
    (test_address(sandbox), new_account(sandbox, "test1"))
}

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

}