use serde_json::Value;
use soroban_test::{AssertExt, TestEnv};

use crate::integration::{
    token::{add_trustline, deploy_sac, sac_balance, sac_id},
    util::{new_account, test_address},
};

#[tokio::test]
async fn set_admin_transfers_control_and_returns_receipt() {
    let sandbox = &TestEnv::new();
    let test = test_address(sandbox);
    let issuer = new_account(sandbox, "issuer");
    let new_admin = new_account(sandbox, "newadmin");
    let asset = format!("USDC:{issuer}");

    add_trustline(sandbox, "test", &asset);
    deploy_sac(sandbox, &asset, "issuer");

    let stdout = sandbox
        .new_assert_cmd("token")
        .args([
            "set-admin",
            "--id",
            &asset,
            "--admin",
            "issuer",
            "--new-admin",
            &new_admin,
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

    // Control has transferred: the new admin can now mint, proving the change
    // took effect.
    sandbox
        .new_assert_cmd("token")
        .args([
            "mint", "--id", &asset, "--admin", "newadmin", "--to", &test, "--amount", "9000000",
        ])
        .assert()
        .success();
    let sac = sac_id(sandbox, &asset);
    assert_eq!(
        sac_balance(sandbox, &sac, &test),
        9_000_000,
        "the new admin should be able to mint"
    );
}

#[tokio::test]
async fn set_admin_fails_when_sac_not_deployed() {
    let sandbox = &TestEnv::new();
    let issuer = new_account(sandbox, "issuer");
    let new_admin = new_account(sandbox, "newadmin");
    let asset = format!("USDC:{issuer}");

    // No SAC deployed → structured deploy-pointer error with a typed discriminator.
    let stdout = sandbox
        .new_assert_cmd("token")
        .args([
            "set-admin",
            "--id",
            &asset,
            "--admin",
            "issuer",
            "--new-admin",
            &new_admin,
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
async fn set_admin_rejects_muxed_source_with_clear_error() {
    let sandbox = &TestEnv::new();
    let new_admin = new_account(sandbox, "newadmin");

    // Muxed (M…) source accounts aren't supported by the invoke pipeline yet
    // (see #2645). Until then the command must reject them up front with a clear
    // message rather than a raw strkey decode error deep in the pipeline.
    let muxed = "MA3D5KRYM6CB7OWQ6TWYRR3Z4T7GNZLKERYNZGGA5SOAOPIFY6YQGAAAAAAAAAPCICBKU";
    sandbox
        .new_assert_cmd("token")
        .args([
            "set-admin",
            "--id",
            "native",
            "--admin",
            muxed,
            "--new-admin",
            &new_admin,
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "muxed (M…) source accounts are not yet supported",
        ));
}
