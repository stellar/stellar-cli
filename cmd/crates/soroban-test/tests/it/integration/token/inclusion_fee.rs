use soroban_cli::commands::tx::fetch::fee::FeeTable;
use soroban_test::{AssertExt, TestEnv};

use crate::integration::{
    token::{add_trustline, deploy_sac, issuer_pays},
    util::{new_account, test_address},
};

/// The configured inclusion fee the submitting token commands must honor, like
/// `contract invoke` and `token mint` already do (#2776).
const INCLUSION_FEE: &str = "5000";

/// Fetch the proposed inclusion fee of the submitted transaction `tx_hash`.
fn proposed_inclusion_fee(sandbox: &TestEnv, tx_hash: &str) -> i64 {
    let stdout = sandbox
        .new_assert_cmd("tx")
        .args(["fetch", "fee", "--hash", tx_hash, "--output", "json"])
        .assert()
        .success()
        .stdout_as_str();
    let table: FeeTable = serde_json::from_str(&stdout).unwrap();
    table.proposed.inclusion_fee
}

/// Run a submitting `token` command, read the tx hash from its JSON receipt, and
/// return the proposed inclusion fee of the resulting transaction.
fn submit_token_tx(sandbox: &TestEnv, args: &[&str], envs: &[(&str, &str)]) -> i64 {
    let mut cmd = sandbox.new_assert_cmd("token");
    for (key, value) in envs {
        cmd.env(key, value);
    }
    let stdout = cmd
        .args(args)
        .args(["--output", "json"])
        .assert()
        .success()
        .stdout_as_str();
    let receipt: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    proposed_inclusion_fee(sandbox, receipt["tx_hash"].as_str().unwrap())
}

/// Submit with `STELLAR_INCLUSION_FEE` set directly. Per-process env keeps this
/// race-free under parallel tests.
fn submit_and_read_inclusion_fee(sandbox: &TestEnv, args: &[&str]) -> i64 {
    submit_token_tx(sandbox, args, &[("STELLAR_INCLUSION_FEE", INCLUSION_FEE)])
}

/// Have `test` approve `spender` for `amount` of the token's SAC, so a delegated
/// `transfer-from`/`burn-from` has an allowance to draw on.
async fn approve(sandbox: &TestEnv, asset: &str, spender: &str, amount: i128) {
    let seq = sandbox.client().get_latest_ledger().await.unwrap().sequence;
    sandbox
        .new_assert_cmd("token")
        .args([
            "approve",
            "--id",
            asset,
            "--from",
            "test",
            "--spender",
            spender,
            "--amount",
            &amount.to_string(),
            "--expiration-ledger",
            &(seq + 1000).to_string(),
        ])
        .assert()
        .success();
}

#[tokio::test]
async fn transfer_honors_configured_inclusion_fee() {
    let sandbox = &TestEnv::new();
    let bob = new_account(sandbox, "bob");

    let inclusion_fee = submit_and_read_inclusion_fee(
        sandbox,
        &[
            "transfer", "--id", "native", "--from", "test", "--to", &bob, "--amount", "1",
        ],
    );
    assert_eq!(inclusion_fee, 5000);
}

#[tokio::test]
async fn burn_honors_configured_inclusion_fee() {
    let sandbox = &TestEnv::new();
    let test = test_address(sandbox);
    let issuer = new_account(sandbox, "issuer");
    let asset = format!("USDC:{issuer}");

    // Burn is invalid on the native asset, so use an issued asset the holder owns.
    add_trustline(sandbox, "test", &asset);
    issuer_pays(sandbox, "issuer", &test, &asset, 1_000);
    deploy_sac(sandbox, &asset, "issuer");

    let inclusion_fee = submit_and_read_inclusion_fee(
        sandbox,
        &["burn", "--id", &asset, "--from", "test", "--amount", "400"],
    );
    assert_eq!(inclusion_fee, 5000);
}

#[tokio::test]
async fn approve_honors_configured_inclusion_fee() {
    let sandbox = &TestEnv::new();
    let issuer = new_account(sandbox, "issuer");
    let spender = new_account(sandbox, "spender");
    let asset = format!("USDC:{issuer}");
    deploy_sac(sandbox, &asset, "issuer");

    let seq = sandbox.client().get_latest_ledger().await.unwrap().sequence;
    let expiration = (seq + 1000).to_string();

    let inclusion_fee = submit_and_read_inclusion_fee(
        sandbox,
        &[
            "approve",
            "--id",
            &asset,
            "--from",
            "test",
            "--spender",
            &spender,
            "--amount",
            "5000000",
            "--expiration-ledger",
            &expiration,
        ],
    );
    assert_eq!(inclusion_fee, 5000);
}

#[tokio::test]
async fn transfer_from_honors_configured_inclusion_fee() {
    let sandbox = &TestEnv::new();
    let test = test_address(sandbox);
    let spender = new_account(sandbox, "spender");
    let recipient = new_account(sandbox, "recipient");
    deploy_sac(sandbox, "native", "test");
    approve(sandbox, "native", &spender, 10_000_000).await;

    // The spender signs, drawing on the allowance to move the owner's funds.
    let inclusion_fee = submit_and_read_inclusion_fee(
        sandbox,
        &[
            "transfer-from",
            "--id",
            "native",
            "--spender",
            "spender",
            "--from",
            &test,
            "--to",
            &recipient,
            "--amount",
            "1",
        ],
    );
    assert_eq!(inclusion_fee, 5000);
}

#[tokio::test]
async fn burn_from_honors_configured_inclusion_fee() {
    let sandbox = &TestEnv::new();
    let test = test_address(sandbox);
    let issuer = new_account(sandbox, "issuer");
    let spender = new_account(sandbox, "spender");
    let asset = format!("USDC:{issuer}");
    add_trustline(sandbox, "test", &asset);
    issuer_pays(sandbox, "issuer", &test, &asset, 1_000);
    deploy_sac(sandbox, &asset, "issuer");
    approve(sandbox, &asset, &spender, 1_000).await;

    // The spender signs, drawing on the allowance to destroy the owner's tokens.
    let inclusion_fee = submit_and_read_inclusion_fee(
        sandbox,
        &[
            "burn-from",
            "--id",
            &asset,
            "--spender",
            "spender",
            "--from",
            &test,
            "--amount",
            "400",
        ],
    );
    assert_eq!(inclusion_fee, 5000);
}

#[tokio::test]
async fn transfer_honors_inclusion_fee_set_with_fees_use() {
    let sandbox = &TestEnv::new();
    let bob = new_account(sandbox, "bob");

    // The primary report: `fees use` routes through the CLI's config→env mirror
    // rather than a directly set `STELLAR_INCLUSION_FEE`. Exercise that path
    // end-to-end with no env override.
    sandbox
        .new_assert_cmd("fees")
        .args(["use", "--amount", "5000"])
        .assert()
        .success();

    let inclusion_fee = submit_token_tx(
        sandbox,
        &[
            "transfer", "--id", "native", "--from", "test", "--to", &bob, "--amount", "1",
        ],
        &[],
    );
    assert_eq!(inclusion_fee, 5000);
}

#[tokio::test]
async fn transfer_rejects_a_malformed_inclusion_fee_env() {
    let sandbox = &TestEnv::new();
    let bob = new_account(sandbox, "bob");

    // A set-but-unparseable fee is rejected up front, matching clap's `env =`
    // resolution for `mint`, rather than silently submitting at the default.
    sandbox
        .new_assert_cmd("token")
        .env("STELLAR_INCLUSION_FEE", "abc")
        .args([
            "transfer", "--id", "native", "--from", "test", "--to", &bob, "--amount", "1",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "STELLAR_INCLUSION_FEE must be a non-negative integer",
        ));
}
