use serde_json::Value;
use soroban_test::{AssertExt, TestEnv};

use crate::integration::{
    token::{add_trustline, deploy_sac, sac_id},
    util::{new_account, test_address},
};

/// Enable the revocable flag on `issuer`, required to deauthorize an existing
/// trustline.
fn enable_revocable(sandbox: &TestEnv, issuer: &str) {
    sandbox
        .new_assert_cmd("tx")
        .args(["new", "set-options", "--set-revocable", "--source", issuer])
        .assert()
        .success();
}

/// Read whether `account` is authorized on the token through its SAC.
fn sac_authorized(sandbox: &TestEnv, contract_id: &str, account: &str) -> bool {
    let stdout = sandbox
        .new_assert_cmd("contract")
        .args([
            "invoke",
            "--id",
            contract_id,
            "--source-account",
            "test",
            "--",
            "authorized",
            "--id",
            account,
        ])
        .assert()
        .success()
        .stdout_as_str();
    stdout.trim().parse().unwrap()
}

#[tokio::test]
async fn set_authorized_toggles_authorization_and_returns_receipt() {
    let sandbox = &TestEnv::new();
    let test = test_address(sandbox);
    let issuer = new_account(sandbox, "issuer");
    let asset = format!("USDC:{issuer}");

    // Deauthorizing an existing trustline requires the issuer to be revocable.
    enable_revocable(sandbox, "issuer");
    add_trustline(sandbox, "test", &asset);
    deploy_sac(sandbox, &asset, "issuer");
    let sac = sac_id(sandbox, &asset);

    // A fresh trustline starts authorized.
    assert!(
        sac_authorized(sandbox, &sac, &test),
        "trustline should start authorized"
    );

    let stdout = sandbox
        .new_assert_cmd("token")
        .args([
            "set-authorized",
            "--id",
            &asset,
            "--admin",
            "issuer",
            "--account",
            &test,
            "--authorize",
            "false",
            "--output",
            "json",
        ])
        .assert()
        .success()
        .stdout_as_str();
    let receipt: Value = serde_json::from_str(&stdout).unwrap();
    assert!(
        receipt["tx_hash"].as_str().is_some(),
        "expected a tx hash, got: {receipt}"
    );

    // The account is now deauthorized on-chain.
    assert!(
        !sac_authorized(sandbox, &sac, &test),
        "account should be deauthorized after set-authorized false"
    );
}

#[tokio::test]
async fn set_authorized_fails_when_sac_not_deployed() {
    let sandbox = &TestEnv::new();
    let test = test_address(sandbox);
    let issuer = new_account(sandbox, "issuer");
    let asset = format!("USDC:{issuer}");

    // No SAC deployed → structured deploy-pointer error with a typed discriminator.
    let stdout = sandbox
        .new_assert_cmd("token")
        .args([
            "set-authorized",
            "--id",
            &asset,
            "--admin",
            "issuer",
            "--account",
            &test,
            "--authorize",
            "true",
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

#[tokio::test]
async fn set_authorized_rejects_muxed_source_with_clear_error() {
    let sandbox = &TestEnv::new();
    let test = test_address(sandbox);

    // Muxed (M…) source accounts aren't supported by the invoke pipeline yet
    // (see #2645). Until then the command must reject them up front with a clear
    // message rather than a raw strkey decode error deep in the pipeline.
    let muxed = "MA3D5KRYM6CB7OWQ6TWYRR3Z4T7GNZLKERYNZGGA5SOAOPIFY6YQGAAAAAAAAAPCICBKU";
    sandbox
        .new_assert_cmd("token")
        .args([
            "set-authorized",
            "--id",
            "native",
            "--admin",
            muxed,
            "--account",
            &test,
            "--authorize",
            "true",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "muxed (M…) source accounts are not yet supported",
        ));
}
