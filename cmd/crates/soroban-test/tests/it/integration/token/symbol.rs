use serde_json::Value;
use soroban_test::{AssertExt, TestEnv};

use crate::integration::{token::deploy_sac, util::new_account};

/// Run `stellar token symbol --id <id> --output json` and return the parsed value.
fn symbol_json(sandbox: &TestEnv, id: &str) -> Value {
    let stdout = sandbox
        .new_assert_cmd("token")
        .args(["symbol", "--id", id, "--output", "json"])
        .assert()
        .success()
        .stdout_as_str();
    serde_json::from_str(&stdout).unwrap()
}

#[tokio::test]
async fn symbol_returns_native_metadata() {
    let sandbox = &TestEnv::new();
    deploy_sac(sandbox, "native", "test");

    let value = symbol_json(sandbox, "native");
    assert_eq!(value["symbol"], "native", "native SAC symbol, got: {value}");
}

#[tokio::test]
async fn symbol_returns_issued_asset_metadata() {
    let sandbox = &TestEnv::new();
    let issuer = new_account(sandbox, "issuer");
    let asset = format!("USDC:{issuer}");
    deploy_sac(sandbox, &asset, "issuer");

    // A SAC's `symbol()` metadata for an issued asset is its asset code.
    let value = symbol_json(sandbox, &asset);
    assert_eq!(
        value["symbol"], "USDC",
        "issued-asset SAC symbol, got: {value}"
    );
}

#[tokio::test]
async fn symbol_fails_when_sac_not_deployed() {
    let sandbox = &TestEnv::new();
    let issuer = new_account(sandbox, "issuer");
    let asset = format!("USDC:{issuer}");

    // No SAC deployed for this asset → structured deploy-pointer error, and in
    // JSON mode the error carries a machine-readable `type` discriminator.
    let stdout = sandbox
        .new_assert_cmd("token")
        .args(["symbol", "--id", &asset, "--output", "json"])
        .assert()
        .failure()
        .stdout_as_str();
    let value: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(
        value["error"]["type"], "sac_not_deployed",
        "expected a typed error, got: {stdout}"
    );
}
