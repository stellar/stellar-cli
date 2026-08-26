pub mod balance;
pub mod transfer;

use soroban_test::{AssertExt, TestEnv};

/// Establish (and auto-authorize) a trustline from `source` to `asset`.
pub fn add_trustline(sandbox: &TestEnv, source: &str, asset: &str) {
    sandbox
        .new_assert_cmd("tx")
        .args(["new", "change-trust", "--line", asset, "--source", source])
        .assert()
        .success();
}

/// Pay `amount` of `asset` from its issuer to `destination`.
pub fn issuer_pays(sandbox: &TestEnv, issuer: &str, destination: &str, asset: &str, amount: i128) {
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "payment",
            "--destination",
            destination,
            "--asset",
            asset,
            "--amount",
            &amount.to_string(),
            "--source",
            issuer,
        ])
        .assert()
        .success();
}

/// Deploy the Stellar Asset Contract for `asset`, tolerating the case where a
/// prior run already deployed it (the SAC is global to the network).
pub fn deploy_sac(sandbox: &TestEnv, asset: &str, source: &str) {
    let output = sandbox
        .new_assert_cmd("contract")
        .args([
            "asset",
            "deploy",
            "--asset",
            asset,
            "--source-account",
            source,
        ])
        .output()
        .expect("failed to run contract asset deploy");

    // A clean deploy succeeds; the only failure we tolerate is the SAC already
    // existing on this (persistent) network. Any other failure is a real bug —
    // surface the captured stderr instead of swallowing it and misdirecting the
    // failure two steps downstream.
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("contract already exists"),
            "contract asset deploy failed: {stderr}"
        );
    }
}

/// The contract id of the SAC for `asset`.
pub fn sac_id(sandbox: &TestEnv, asset: &str) -> String {
    sandbox
        .new_assert_cmd("contract")
        .args(["id", "asset", "--asset", asset])
        .assert()
        .success()
        .stdout_as_str()
}

/// Read a token's balance for `account` through its Stellar Asset Contract,
/// returning the raw stroop amount.
pub fn sac_balance(sandbox: &TestEnv, contract_id: &str, account: &str) -> i128 {
    let stdout = sandbox
        .new_assert_cmd("contract")
        .args([
            "invoke",
            "--id",
            contract_id,
            "--source-account",
            "test",
            "--",
            "balance",
            "--id",
            account,
        ])
        .assert()
        .success()
        .stdout_as_str();
    stdout.trim().trim_matches('"').parse().unwrap()
}
