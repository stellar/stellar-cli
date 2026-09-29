use serde_json::Value;
use soroban_test::{AssertExt, TestEnv};

use crate::integration::{
    token::{add_trustline, deploy_sac, sac_balance, sac_id},
    util::{deploy_hello, new_account, test_address},
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
            "--source",
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
            "mint", "--id", &asset, "--source", "newadmin", "--to", &test, "--amount", "9000000",
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
            "--source",
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
            "--source",
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

#[tokio::test]
async fn set_admin_rejects_muxed_new_admin_with_clear_error() {
    let sandbox = &TestEnv::new();

    // A muxed (M…) successor would be stranded — it can't sign as a source and
    // the SAC stores a plain `Address` — so the command rejects it up front
    // rather than performing an irreversible transfer to an unusable admin.
    let muxed = "MA3D5KRYM6CB7OWQ6TWYRR3Z4T7GNZLKERYNZGGA5SOAOPIFY6YQGAAAAAAAAAPCICBKU";
    sandbox
        .new_assert_cmd("token")
        .args([
            "set-admin",
            "--id",
            "native",
            "--source",
            "test",
            "--new-admin",
            muxed,
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "muxed (M…) new-admin accounts are not yet supported",
        ));
}

#[tokio::test]
async fn set_admin_warns_when_target_is_not_a_sac() {
    let sandbox = &TestEnv::new();
    let new_admin = new_account(sandbox, "newadmin");
    let contract_id = deploy_hello(sandbox).await;

    // Pointing a SAC-admin command at a plain wasm contract warns. The call then
    // fails (hello_world has no `set_admin`), but the heads-up is the point.
    let stderr = sandbox
        .new_assert_cmd("token")
        .args([
            "set-admin",
            "--id",
            &contract_id,
            "--source",
            "test",
            "--new-admin",
            &new_admin,
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
async fn set_admin_does_not_warn_when_target_is_a_sac() {
    let sandbox = &TestEnv::new();
    let issuer = new_account(sandbox, "issuer");
    let new_admin = new_account(sandbox, "newadmin");
    let asset = format!("USDC:{issuer}");

    deploy_sac(sandbox, &asset, "issuer");
    // Reference the SAC by its contract id, not the asset, so the check can only
    // clear it by inspecting the on-chain executable — not the id's text form.
    let sac = sac_id(sandbox, &asset);

    let stderr = sandbox
        .new_assert_cmd("token")
        .args([
            "set-admin",
            "--id",
            &sac,
            "--source",
            "issuer",
            "--new-admin",
            &new_admin,
        ])
        .assert()
        .success()
        .stderr_as_str();
    assert!(
        !stderr.contains("is not a Stellar Asset Contract"),
        "a genuine SAC should not warn, got: {stderr}"
    );
}
