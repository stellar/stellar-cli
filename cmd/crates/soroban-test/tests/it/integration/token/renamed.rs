//! `stellar token` against a SEP-41-shaped contract whose parameters use
//! non-canonical names (`balance(who)`, `transfer(sender, recipient, amt)`).
//! These prove the command maps values to the contract's parameters by
//! position, not by name — the old flag-name mapping would fail here.
use serde_json::Value;
use soroban_test::{AssertExt, TestEnv, Wasm};

use crate::integration::util::{deploy_contract, new_account, test_address, DeployOptions};

const TOKEN_RENAMED: &Wasm = &Wasm::Custom("test-wasms", "test_token_renamed");

/// Deploy the renamed-arg token and set its `decimals` to `decimal_count`,
/// returning the contract id. No balance is seeded.
async fn deploy_with_decimals(sandbox: &TestEnv, decimal_count: u32) -> String {
    let id = deploy_contract(sandbox, TOKEN_RENAMED, DeployOptions::default()).await;
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
            &decimal_count.to_string(),
        ])
        .assert()
        .success();
    id
}

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
async fn balance_decimal_rejects_oversized_decimals() {
    let sandbox = &TestEnv::new();
    let test = test_address(sandbox);
    // `decimals` is contract-controlled; a hostile value would make `--decimal`
    // pad the output to that many characters. Anything past the CLI's cap is
    // rejected up front rather than risking a pathological allocation. 1000 is
    // over the cap but small enough that, without the guard, formatting would
    // still succeed — so this test truly catches a regression.
    let id = deploy_with_decimals(sandbox, 1000).await;

    let stdout = sandbox
        .new_assert_cmd("token")
        .args([
            "balance",
            "--id",
            &id,
            "--account",
            &test,
            "--decimal",
            "--output",
            "json",
        ])
        .assert()
        .failure()
        .stdout_as_str();
    let value: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(
        value["error"]["type"], "decimals_too_large",
        "expected a typed error, got: {stdout}"
    );
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
