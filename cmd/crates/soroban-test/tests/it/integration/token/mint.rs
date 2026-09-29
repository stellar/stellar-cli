use serde_json::Value;
use soroban_test::{AssertExt, TestEnv};

use crate::integration::{
    token::{add_trustline, deploy_sac, sac_balance, sac_id},
    util::{deploy_hello, new_account, test_address},
};

#[tokio::test]
async fn mint_credits_recipient_and_returns_receipt() {
    let sandbox = &TestEnv::new();
    let test = test_address(sandbox);
    let issuer = new_account(sandbox, "issuer");
    let asset = format!("USDC:{issuer}");

    // The SAC admin is the asset issuer. Minting a classic asset to an account
    // requires the recipient to hold a trustline.
    add_trustline(sandbox, "test", &asset);
    deploy_sac(sandbox, &asset, "issuer");

    let stdout = sandbox
        .new_assert_cmd("token")
        .args([
            "mint", "--id", &asset, "--source", "issuer", "--to", &test, "--amount", "7500000",
            "--output", "json",
        ])
        .assert()
        .success()
        .stdout_as_str();
    let receipt: Value = serde_json::from_str(&stdout).unwrap();
    assert!(
        receipt["tx_hash"].as_str().is_some(),
        "expected a tx hash, got: {receipt}"
    );

    // The minted balance is now readable on-chain.
    let sac = sac_id(sandbox, &asset);
    assert_eq!(
        sac_balance(sandbox, &sac, &test),
        7_500_000,
        "expected the minted balance on-chain"
    );
}

#[tokio::test]
async fn mint_fails_when_sac_not_deployed() {
    let sandbox = &TestEnv::new();
    let test = test_address(sandbox);
    let issuer = new_account(sandbox, "issuer");
    let asset = format!("USDC:{issuer}");

    // No SAC deployed → structured deploy-pointer error with a typed discriminator.
    let stdout = sandbox
        .new_assert_cmd("token")
        .args([
            "mint", "--id", &asset, "--source", "issuer", "--to", &test, "--amount", "1",
            "--output", "json",
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
async fn mint_rejects_negative_amount_before_any_rpc() {
    let sandbox = &TestEnv::new();
    let test = test_address(sandbox);

    // A negative mint is rejected at the CLI layer, before any network call.
    // `=` form so clap reads `-1` as the value, not an unknown flag.
    sandbox
        .new_assert_cmd("token")
        .args([
            "mint",
            "--id",
            "native",
            "--source",
            "test",
            "--to",
            &test,
            "--amount=-1",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains("must not be negative"));
}

#[tokio::test]
async fn mint_rejects_muxed_source_with_clear_error() {
    let sandbox = &TestEnv::new();
    let test = test_address(sandbox);

    // Muxed (M…) source accounts aren't supported by the invoke pipeline yet
    // (see #2645). Until then the command must reject them up front with a clear
    // message rather than a raw strkey decode error deep in the pipeline.
    let muxed = "MA3D5KRYM6CB7OWQ6TWYRR3Z4T7GNZLKERYNZGGA5SOAOPIFY6YQGAAAAAAAAAPCICBKU";
    sandbox
        .new_assert_cmd("token")
        .args([
            "mint", "--id", "native", "--source", muxed, "--to", &test, "--amount", "1",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "muxed (M…) source accounts are not yet supported",
        ));
}

#[tokio::test]
async fn mint_warns_when_target_is_not_a_sac() {
    let sandbox = &TestEnv::new();
    let test = test_address(sandbox);
    let contract_id = deploy_hello(sandbox).await;

    // Pointing a SAC-admin command at a plain wasm contract warns. The mint then
    // fails (hello_world has no `mint`), but the heads-up on stderr is the point.
    let stderr = sandbox
        .new_assert_cmd("token")
        .args([
            "mint",
            "--id",
            &contract_id,
            "--source",
            "test",
            "--to",
            &test,
            "--amount",
            "1",
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
async fn mint_does_not_warn_when_target_is_a_sac() {
    let sandbox = &TestEnv::new();
    let test = test_address(sandbox);
    let issuer = new_account(sandbox, "issuer");
    let asset = format!("USDC:{issuer}");

    add_trustline(sandbox, "test", &asset);
    deploy_sac(sandbox, &asset, "issuer");
    // Reference the SAC by its contract id, not the asset, so the check can only
    // clear it by inspecting the on-chain executable — not the id's text form.
    let sac = sac_id(sandbox, &asset);

    let stderr = sandbox
        .new_assert_cmd("token")
        .args([
            "mint", "--id", &sac, "--source", "issuer", "--to", &test, "--amount", "1",
        ])
        .assert()
        .success()
        .stderr_as_str();
    assert!(
        !stderr.contains("is not a Stellar Asset Contract"),
        "a genuine SAC should not warn, got: {stderr}"
    );
}
