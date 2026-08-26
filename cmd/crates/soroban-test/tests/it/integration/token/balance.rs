use serde_json::Value;
use soroban_test::{AssertExt, TestEnv};

use crate::integration::{
    token::{add_trustline, deploy_sac, issuer_pays},
    util::{new_account, test_address},
};

/// Run `stellar token balance ... --output json` and return the parsed value.
fn balance_json(sandbox: &TestEnv, id: &str, account: &str, decimal: bool) -> Value {
    let mut args = vec![
        "balance",
        "--id",
        id,
        "--account",
        account,
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

#[tokio::test]
async fn balance_returns_stroops_and_optional_decimal() {
    let sandbox = &TestEnv::new();
    let test = test_address(sandbox);
    let issuer = new_account(sandbox, "issuer");
    let asset = format!("USDC:{issuer}");

    // Give `test` a known balance: 12_300_000 stroops == 1.23 units (7 decimals).
    add_trustline(sandbox, "test", &asset);
    issuer_pays(sandbox, "issuer", &test, &asset, 12_300_000);
    deploy_sac(sandbox, &asset, "issuer");

    // Default: raw stroops, as a string.
    let raw = balance_json(sandbox, &asset, "test", false);
    assert_eq!(raw["balance"], "12300000", "raw balance, got: {raw}");
    assert!(
        raw.get("decimals").is_none(),
        "no decimals without --decimal"
    );

    // `--decimal`: decimal-aware value plus the token's decimals.
    let dec = balance_json(sandbox, &asset, "test", true);
    assert_eq!(dec["balance"], "1.23", "decimal balance, got: {dec}");
    assert_eq!(dec["decimals"], 7, "decimals, got: {dec}");
}

#[tokio::test]
async fn balance_zero_for_account_without_holdings() {
    let sandbox = &TestEnv::new();
    let issuer = new_account(sandbox, "issuer");
    let asset = format!("USDC:{issuer}");

    // A holder with a trustline but no issuance holds a zero balance.
    let holder = new_account(sandbox, "holder");
    add_trustline(sandbox, "holder", &asset);
    deploy_sac(sandbox, &asset, "issuer");

    let raw = balance_json(sandbox, &asset, &holder, false);
    assert_eq!(raw["balance"], "0", "expected zero balance, got: {raw}");
}

#[tokio::test]
async fn balance_fails_when_sac_not_deployed() {
    let sandbox = &TestEnv::new();
    let issuer = new_account(sandbox, "issuer");
    let asset = format!("USDC:{issuer}");

    // No SAC deployed for this asset → structured deploy-pointer error.
    sandbox
        .new_assert_cmd("token")
        .args(["balance", "--id", &asset, "--account", "test"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("contract asset deploy"));

    // In JSON mode the error carries a machine-readable `type` discriminator.
    let stdout = sandbox
        .new_assert_cmd("token")
        .args([
            "balance",
            "--id",
            &asset,
            "--account",
            "test",
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
    assert!(
        value["error"]["message"].as_str().is_some(),
        "expected an error message, got: {stdout}"
    );
}

#[tokio::test]
async fn balance_fails_when_contract_not_found() {
    let sandbox = &TestEnv::new();
    // A well-formed contract id that resolves to a plain contract (not a SAC)
    // and was never deployed → the missing-contract branch, distinct from the
    // SAC deploy pointer above.
    let id = "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAFCT4";

    // In JSON mode the error carries a machine-readable `type` discriminator.
    let stdout = sandbox
        .new_assert_cmd("token")
        .args([
            "balance",
            "--id",
            id,
            "--account",
            "test",
            "--output",
            "json",
        ])
        .assert()
        .failure()
        .stdout_as_str();
    let value: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(
        value["error"]["type"], "contract_not_found",
        "expected a typed error, got: {stdout}"
    );
    assert!(
        value["error"]["message"].as_str().is_some(),
        "expected an error message, got: {stdout}"
    );
}
