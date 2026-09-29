use serde_json::Value;
use soroban_test::{AssertExt, TestEnv};

use crate::integration::{
    token::{add_trustline, deploy_sac, issuer_pays, sac_balance, sac_id},
    util::{deploy_hello, new_account, test_address},
};

/// Enable the clawback flag on `issuer`, so trustlines created afterwards are
/// clawback-enabled. `AUTH_CLAWBACK_ENABLED` requires `AUTH_REVOCABLE`, so set
/// both together.
fn enable_clawback(sandbox: &TestEnv, issuer: &str) {
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "set-options",
            "--set-revocable",
            "--set-clawback-enabled",
            "--source",
            issuer,
        ])
        .assert()
        .success();
}

#[tokio::test]
async fn clawback_removes_balance_and_returns_receipt() {
    let sandbox = &TestEnv::new();
    let test = test_address(sandbox);
    let issuer = new_account(sandbox, "issuer");
    let asset = format!("USDC:{issuer}");

    // Clawback requires the issuer to enable the flag *before* the holder's
    // trustline exists, so the trustline is created clawback-enabled.
    enable_clawback(sandbox, "issuer");
    add_trustline(sandbox, "test", &asset);
    deploy_sac(sandbox, &asset, "issuer");
    issuer_pays(sandbox, "issuer", &test, &asset, 10_000_000);

    let stdout = sandbox
        .new_assert_cmd("token")
        .args([
            "clawback", "--id", &asset, "--source", "issuer", "--from", &test, "--amount",
            "4000000", "--output", "json",
        ])
        .assert()
        .success()
        .stdout_as_str();
    let receipt: Value = serde_json::from_str(&stdout).unwrap();
    assert!(
        receipt["tx_hash"].as_str().is_some(),
        "expected a tx hash, got: {receipt}"
    );

    // 10_000_000 minted − 4_000_000 clawed back = 6_000_000 remaining.
    let sac = sac_id(sandbox, &asset);
    assert_eq!(
        sac_balance(sandbox, &sac, &test),
        6_000_000,
        "expected the remaining balance after clawback"
    );
}

#[tokio::test]
async fn clawback_fails_when_sac_not_deployed() {
    let sandbox = &TestEnv::new();
    let test = test_address(sandbox);
    let issuer = new_account(sandbox, "issuer");
    let asset = format!("USDC:{issuer}");

    // No SAC deployed → structured deploy-pointer error with a typed discriminator.
    let stdout = sandbox
        .new_assert_cmd("token")
        .args([
            "clawback", "--id", &asset, "--source", "issuer", "--from", &test, "--amount", "1",
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
async fn clawback_rejects_muxed_source_with_clear_error() {
    let sandbox = &TestEnv::new();
    let holder = new_account(sandbox, "holder");

    // Muxed (M…) source accounts aren't supported by the invoke pipeline yet
    // (see #2645). Until then the command must reject them up front with a clear
    // message rather than a raw strkey decode error deep in the pipeline.
    let muxed = "MA3D5KRYM6CB7OWQ6TWYRR3Z4T7GNZLKERYNZGGA5SOAOPIFY6YQGAAAAAAAAAPCICBKU";
    sandbox
        .new_assert_cmd("token")
        .args([
            "clawback", "--id", "native", "--source", muxed, "--from", &holder, "--amount", "1",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "muxed (M…) source accounts are not yet supported",
        ));
}

#[tokio::test]
async fn clawback_rejects_muxed_from_with_clear_error() {
    let sandbox = &TestEnv::new();

    // A muxed (M…) holder isn't a valid `clawback` target — the host rejects it
    // mid-simulation with an opaque error — so the command rejects it up front
    // with a clear message.
    let muxed = "MA3D5KRYM6CB7OWQ6TWYRR3Z4T7GNZLKERYNZGGA5SOAOPIFY6YQGAAAAAAAAAPCICBKU";
    sandbox
        .new_assert_cmd("token")
        .args([
            "clawback", "--id", "native", "--source", "test", "--from", muxed, "--amount", "1",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "muxed (M…) holder accounts are not yet supported",
        ));
}

#[tokio::test]
async fn clawback_rejects_negative_amount_before_any_rpc() {
    let sandbox = &TestEnv::new();
    let test = test_address(sandbox);

    // A negative clawback is rejected at the CLI layer, before any network call.
    // `=` form so clap reads `-1` as the value, not an unknown flag.
    sandbox
        .new_assert_cmd("token")
        .args([
            "clawback",
            "--id",
            "native",
            "--source",
            "test",
            "--from",
            &test,
            "--amount=-1",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains("must not be negative"));
}

#[tokio::test]
async fn clawback_warns_when_target_is_not_a_sac() {
    let sandbox = &TestEnv::new();
    let test = test_address(sandbox);
    let contract_id = deploy_hello(sandbox).await;

    // Pointing a SAC-admin command at a plain wasm contract warns. The clawback
    // then fails (hello_world has no `clawback`), but the heads-up is the point.
    let stderr = sandbox
        .new_assert_cmd("token")
        .args([
            "clawback",
            "--id",
            &contract_id,
            "--source",
            "test",
            "--from",
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
async fn clawback_does_not_warn_when_target_is_a_sac() {
    let sandbox = &TestEnv::new();
    let test = test_address(sandbox);
    let issuer = new_account(sandbox, "issuer");
    let asset = format!("USDC:{issuer}");

    enable_clawback(sandbox, "issuer");
    add_trustline(sandbox, "test", &asset);
    deploy_sac(sandbox, &asset, "issuer");
    issuer_pays(sandbox, "issuer", &test, &asset, 10_000_000);
    // Reference the SAC by its contract id, not the asset, so the check can only
    // clear it by inspecting the on-chain executable — not the id's text form.
    let sac = sac_id(sandbox, &asset);

    let stderr = sandbox
        .new_assert_cmd("token")
        .args([
            "clawback", "--id", &sac, "--source", "issuer", "--from", &test, "--amount", "4000000",
        ])
        .assert()
        .success()
        .stderr_as_str();
    assert!(
        !stderr.contains("is not a Stellar Asset Contract"),
        "a genuine SAC should not warn, got: {stderr}"
    );
}
