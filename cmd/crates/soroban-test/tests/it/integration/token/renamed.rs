//! `stellar token` against a SEP-41-shaped contract whose parameters use
//! non-canonical names (`balance(who)`, `transfer(sender, recipient, amt)`).
//! These prove the command maps values to the contract's parameters by
//! position, not by name — the old flag-name mapping would fail here.
use soroban_test::{AssertExt, TestEnv, Wasm};

use crate::integration::util::{deploy_contract, new_account, test_address, DeployOptions};

const TOKEN_RENAMED: &Wasm = &Wasm::Custom("test-wasms", "test_token_renamed");

/// Deploy the renamed-arg token, set decimals to 7, and mint `qty` to `test`.
/// Returns the contract id and `test`'s address.
async fn deploy_and_seed(sandbox: &TestEnv, qty: i128) -> (String, String) {
    let id = deploy_contract(sandbox, TOKEN_RENAMED, DeployOptions::default()).await;
    let test = test_address(sandbox);

    sandbox
        .new_assert_cmd("contract")
        .args([
            "invoke",
            "--id",
            &id,
            "--source-account",
            "test",
            "--",
            "init",
            "--decimal_count",
            "7",
        ])
        .assert()
        .success();

    sandbox
        .new_assert_cmd("contract")
        .args([
            "invoke",
            "--id",
            &id,
            "--source-account",
            "test",
            "--",
            "mint",
            "--dest",
            &test,
            "--qty",
            &qty.to_string(),
        ])
        .assert()
        .success();

    (id, test)
}

/// Read a balance through the `stellar token balance` command under test.
fn token_balance(sandbox: &TestEnv, id: &str, account: &str) -> String {
    sandbox
        .new_assert_cmd("token")
        .args(["balance", "--id", id, "--account", account])
        .assert()
        .success()
        .stdout_as_str()
        .trim()
        .to_string()
}

#[tokio::test]
async fn transfer_maps_renamed_params_by_position() {
    let sandbox = &TestEnv::new();
    let (id, _test) = deploy_and_seed(sandbox, 1_000_000).await;
    let recipient = new_account(sandbox, "recipient");

    // The contract's `transfer` params are sender/recipient/amt, not
    // from/to/amount — this only works if values map by position.
    sandbox
        .new_assert_cmd("token")
        .args([
            "transfer", "--id", &id, "--to", &recipient, "--amount", "400", "--from", "test",
        ])
        .assert()
        .success();

    assert_eq!(token_balance(sandbox, &id, &recipient), "400");
}

#[tokio::test]
async fn balance_reads_renamed_param_and_applies_decimals() {
    let sandbox = &TestEnv::new();
    let (id, test) = deploy_and_seed(sandbox, 1_000_000).await;

    // `balance`'s param is `who`, not `id`.
    assert_eq!(token_balance(sandbox, &id, &test), "1000000");

    // `--decimal` runs a second `decimals()` call (no args) and scales the raw
    // amount: 1_000_000 with 7 decimals renders as 0.1.
    let decimal = sandbox
        .new_assert_cmd("token")
        .args(["balance", "--id", &id, "--account", &test, "--decimal"])
        .assert()
        .success()
        .stdout_as_str();
    assert!(
        decimal.trim().starts_with("0.1"),
        "expected a decimal balance starting with 0.1, got: {decimal:?}"
    );
}
