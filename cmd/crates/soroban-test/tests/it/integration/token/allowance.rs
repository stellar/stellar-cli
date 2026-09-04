use serde_json::Value;
use soroban_test::{AssertExt, TestEnv};

use crate::integration::{
    token::deploy_sac,
    util::{new_account, test_address},
};

/// Run `stellar token allowance ... --output json` and return the parsed value.
fn allowance_json(sandbox: &TestEnv, id: &str, from: &str, spender: &str, decimal: bool) -> Value {
    let mut args = vec![
        "allowance",
        "--id",
        id,
        "--from",
        from,
        "--spender",
        spender,
        "--output",
        "json",
    ];
    if decimal {
        args.push("--decimal");
    }
    let stdout = sandbox
        .new_assert_cmd("token")
        .args(args)
        .assert()
        .success()
        .stdout_as_str();
    serde_json::from_str(&stdout).unwrap()
}

/// Grant `spender` an allowance of `amount` on `asset`, authorized by `test`.
fn approve(sandbox: &TestEnv, asset: &str, spender: &str, amount: i128, expiration: u32) {
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
            &expiration.to_string(),
        ])
        .assert()
        .success();
}

#[tokio::test]
async fn allowance_returns_stroops_and_optional_decimal() {
    let sandbox = &TestEnv::new();
    let test = test_address(sandbox);
    let issuer = new_account(sandbox, "issuer");
    let spender = new_account(sandbox, "spender");
    let asset = format!("USDC:{issuer}");
    deploy_sac(sandbox, &asset, "issuer");

    // Approve 1.23 units (12_300_000 stroops at 7 decimals).
    let seq = sandbox.client().get_latest_ledger().await.unwrap().sequence;
    approve(sandbox, &asset, &spender, 12_300_000, seq + 1000);

    // Default: raw stroops, as a string.
    let raw = allowance_json(sandbox, &asset, &test, &spender, false);
    assert_eq!(raw["allowance"], "12300000", "raw allowance, got: {raw}");
    assert!(
        raw.get("decimals").is_none(),
        "no decimals without --decimal"
    );

    // `--decimal`: decimal-aware value plus the token's decimals.
    let dec = allowance_json(sandbox, &asset, &test, &spender, true);
    assert_eq!(dec["allowance"], "1.23", "decimal allowance, got: {dec}");
    assert_eq!(dec["decimals"], 7, "decimals, got: {dec}");
}

#[tokio::test]
async fn allowance_zero_when_never_approved() {
    let sandbox = &TestEnv::new();
    let test = test_address(sandbox);
    let issuer = new_account(sandbox, "issuer");
    let spender = new_account(sandbox, "spender");
    let asset = format!("USDC:{issuer}");
    deploy_sac(sandbox, &asset, "issuer");

    // No approval granted → a zero allowance.
    let raw = allowance_json(sandbox, &asset, &test, &spender, false);
    assert_eq!(raw["allowance"], "0", "expected zero allowance, got: {raw}");
}

#[tokio::test]
async fn allowance_fails_when_sac_not_deployed() {
    let sandbox = &TestEnv::new();
    let issuer = new_account(sandbox, "issuer");
    let spender = new_account(sandbox, "spender");
    let asset = format!("USDC:{issuer}");

    // No SAC deployed → structured deploy-pointer error with a typed discriminator.
    let stdout = sandbox
        .new_assert_cmd("token")
        .args([
            "allowance",
            "--id",
            &asset,
            "--from",
            "test",
            "--spender",
            &spender,
            "--output",
            "json",
        ])
        .assert()
        .failure()
        .stdout_as_str();
    let value: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(
        value["error"]["type"], "sac_not_deployed",
        "expected a typed error, got: {stdout}"
    );
}
