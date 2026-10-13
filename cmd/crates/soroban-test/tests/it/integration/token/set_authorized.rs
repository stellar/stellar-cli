use serde_json::Value;
use soroban_test::{AssertExt, TestEnv};

use crate::integration::{
    token::{add_trustline, deploy_sac, sac_id},
    util::{deploy_hello, new_account, test_address},
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
            "--source",
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

    sandbox
        .new_assert_cmd("token")
        .args([
            "set-authorized",
            "--id",
            &asset,
            "--source",
            "issuer",
            "--account",
            &test,
            "--authorize",
            "true",
        ])
        .assert()
        .success();

    assert!(
        sac_authorized(sandbox, &sac, &test),
        "account should be reauthorized after set-authorized true"
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
            "--source",
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
            "--source",
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

#[tokio::test]
async fn set_authorized_rejects_muxed_account_with_clear_error() {
    let sandbox = &TestEnv::new();

    // The SAC `set_authorized` target is a plain `Address`; a muxed (M…) account
    // is rejected mid-simulation with an opaque error, so the command must reject
    // one up front with a clear message.
    let muxed = "MA3D5KRYM6CB7OWQ6TWYRR3Z4T7GNZLKERYNZGGA5SOAOPIFY6YQGAAAAAAAAAPCICBKU";
    sandbox
        .new_assert_cmd("token")
        .args([
            "set-authorized",
            "--id",
            "native",
            "--source",
            "test",
            "--account",
            muxed,
            "--authorize",
            "true",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "muxed (M…) accounts are not yet supported",
        ));
}

#[tokio::test]
async fn set_authorized_warns_when_target_is_not_a_sac() {
    let sandbox = &TestEnv::new();
    let test = test_address(sandbox);
    let contract_id = deploy_hello(sandbox).await;

    // Pointing a SAC-admin command at a plain wasm contract warns. The call then
    // fails (hello_world has no `set_authorized`), but the heads-up is the point.
    let stderr = sandbox
        .new_assert_cmd("token")
        .args([
            "set-authorized",
            "--id",
            &contract_id,
            "--source",
            "test",
            "--account",
            &test,
            "--authorize",
            "true",
        ])
        .assert()
        .failure()
        .stderr_as_str();
    assert!(
        stderr.contains("is not a Stellar Asset Contract"),
        "expected a non-SAC warning, got: {stderr}"
    );
}

#[tokio::test]
async fn set_authorized_does_not_warn_when_target_is_a_sac() {
    let sandbox = &TestEnv::new();
    let test = test_address(sandbox);
    let issuer = new_account(sandbox, "issuer");
    let asset = format!("USDC:{issuer}");

    add_trustline(sandbox, "test", &asset);
    deploy_sac(sandbox, &asset, "issuer");
    // Reference the SAC by its contract id, not the asset, so the check can only
    // clear it by inspecting the on-chain executable — not the id's text form.
    let sac = sac_id(sandbox, &asset);

    // Re-authorizing an already-authorized trustline is a no-op success and needs
    // no revocable flag — enough to exercise the SAC path without warning.
    let stderr = sandbox
        .new_assert_cmd("token")
        .args([
            "set-authorized",
            "--id",
            &sac,
            "--source",
            "issuer",
            "--account",
            &test,
            "--authorize",
            "true",
        ])
        .assert()
        .success()
        .stderr_as_str();
    assert!(
        !stderr.contains("is not a Stellar Asset Contract"),
        "a genuine SAC should not warn, got: {stderr}"
    );
}
