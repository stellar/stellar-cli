use predicates::prelude::*;
use serde_json::Value;
use soroban_test::{AssertExt, TestEnv};

use crate::integration::{
    token::{add_trustline, deploy_sac, issuer_pays, sac_balance, sac_id},
    util::{new_account, test_address},
};

/// Run `stellar token transfer ... --output json` and return the parsed receipt.
///
/// Also asserts that JSON mode keeps stdout pure JSON and does not leak the
/// invoke pipeline's human-readable status logging onto stderr.
fn transfer_json(sandbox: &TestEnv, id: &str, to: &str, amount: i128) -> Value {
    let stdout = sandbox
        .new_assert_cmd("token")
        .args([
            "transfer",
            "--id",
            id,
            "--to",
            to,
            "--amount",
            &amount.to_string(),
            "--output",
            "json",
            "--from",
            "test",
        ])
        .assert()
        .success()
        .stderr(predicate::str::contains("Simulating transaction").not())
        .stderr(predicate::str::contains("Sending transaction").not())
        .stdout_as_str();
    serde_json::from_str(&stdout).unwrap()
}

#[tokio::test]
async fn transfer_native_returns_receipt_and_moves_funds() {
    let sandbox = &TestEnv::new();
    let recipient = new_account(sandbox, "recipient");

    // Ensure the native SAC exists so `--id native` resolves to a live contract.
    deploy_sac(sandbox, "native", "test");
    let native_id = sac_id(sandbox, "native");

    let amount: i128 = 10_000_000;
    let before = sac_balance(sandbox, &native_id, &recipient);

    let receipt = transfer_json(sandbox, "native", &recipient, amount);
    assert!(
        receipt["tx_hash"].as_str().is_some_and(|h| !h.is_empty()),
        "expected a non-empty tx_hash, got: {receipt}"
    );

    let after = sac_balance(sandbox, &native_id, &recipient);
    assert_eq!(after, before + amount, "recipient balance should increase");
}

#[tokio::test]
async fn transfer_to_contract_destination_succeeds() {
    let sandbox = &TestEnv::new();

    deploy_sac(sandbox, "native", "test");
    let native_id = sac_id(sandbox, "native");

    // Any deployed contract can hold SAC balances (no trustline needed), so use
    // another SAC's contract id as a `C…` destination — this exercises the
    // contract-address path that `--to` must accept.
    let issuer = new_account(sandbox, "issuer");
    let asset = format!("USDC:{issuer}");
    deploy_sac(sandbox, &asset, "issuer");
    let contract_dest = sac_id(sandbox, &asset);

    let amount: i128 = 5_000_000;
    let before = sac_balance(sandbox, &native_id, &contract_dest);

    let receipt = transfer_json(sandbox, "native", &contract_dest, amount);
    assert!(
        receipt["tx_hash"].as_str().is_some_and(|h| !h.is_empty()),
        "expected a non-empty tx_hash, got: {receipt}"
    );

    let after = sac_balance(sandbox, &native_id, &contract_dest);
    assert_eq!(
        after,
        before + amount,
        "contract recipient balance should increase"
    );
}

#[tokio::test]
async fn transfer_issued_asset_succeeds_with_deployed_sac_and_trustlines() {
    let sandbox = &TestEnv::new();
    let test = test_address(sandbox);
    let issuer = new_account(sandbox, "issuer");
    let recipient = new_account(sandbox, "recipient");
    let asset = format!("USDC:{issuer}");

    // Both the holder (`test`) and the recipient need trustlines; the holder is
    // funded by the issuer so it has a balance to send.
    add_trustline(sandbox, "test", &asset);
    add_trustline(sandbox, "recipient", &asset);
    issuer_pays(sandbox, "issuer", &test, &asset, 1_000);

    deploy_sac(sandbox, &asset, "issuer");
    let contract_id = sac_id(sandbox, &asset);

    let amount: i128 = 400;
    let before = sac_balance(sandbox, &contract_id, &recipient);

    let receipt = transfer_json(sandbox, &asset, &recipient, amount);
    assert!(
        receipt["tx_hash"].as_str().is_some_and(|h| !h.is_empty()),
        "expected a non-empty tx_hash, got: {receipt}"
    );

    let after = sac_balance(sandbox, &contract_id, &recipient);
    assert_eq!(after, before + amount, "recipient balance should increase");
}

#[tokio::test]
async fn transfer_fails_when_sac_not_deployed() {
    let sandbox = &TestEnv::new();
    let issuer = new_account(sandbox, "issuer");
    let recipient = new_account(sandbox, "recipient");
    let asset = format!("USDC:{issuer}");

    // The SAC for this issued asset was never deployed, so the transfer must
    // fail with a structured error pointing at `contract asset deploy` rather
    // than silently succeeding or leaking a raw RPC error.
    sandbox
        .new_assert_cmd("token")
        .args([
            "transfer", "--id", &asset, "--to", &recipient, "--amount", "1", "--from", "test",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains("contract asset deploy"));
}

#[tokio::test]
async fn transfer_fails_when_contract_not_found() {
    let sandbox = &TestEnv::new();
    let recipient = new_account(sandbox, "recipient");
    // A well-formed contract id that resolves to a plain contract (not a SAC)
    // and was never deployed → the missing-contract branch, distinct from the
    // SAC deploy pointer above.
    let id = "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAFCT4";

    // In JSON mode the error carries a machine-readable `type` discriminator.
    let stdout = sandbox
        .new_assert_cmd("token")
        .args([
            "transfer", "--id", id, "--to", &recipient, "--amount", "1", "--from", "test",
            "--output", "json",
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

#[tokio::test]
async fn transfer_fails_when_recipient_trustline_missing() {
    let sandbox = &TestEnv::new();
    let test = test_address(sandbox);
    let issuer = new_account(sandbox, "issuer");
    let recipient = new_account(sandbox, "recipient");
    let asset = format!("USDC:{issuer}");

    // The holder can send, but the recipient never established a trustline, so
    // the SAC transfer must fail and name the missing trustline.
    add_trustline(sandbox, "test", &asset);
    issuer_pays(sandbox, "issuer", &test, &asset, 1_000);
    deploy_sac(sandbox, &asset, "issuer");

    sandbox
        .new_assert_cmd("token")
        .args([
            "transfer", "--id", &asset, "--to", &recipient, "--amount", "1", "--from", "test",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains("trustline entry is missing"));
}

#[tokio::test]
async fn transfer_rejects_negative_amount_before_any_rpc() {
    let sandbox = &TestEnv::new();

    // A negative amount is nonsensical and is rejected at the clap layer before
    // any account resolution or RPC work, so this needs no funded accounts and
    // no network — a literal destination address is enough to parse the args.
    let recipient = "GAF4UUODFGAAMYRTF5QKUZCCZPXF3S4PRU5NS2BBRVJGX4WLRVI4ZI4Z";
    sandbox
        .new_assert_cmd("token")
        .args([
            "transfer",
            "--id",
            "native",
            "--to",
            recipient,
            "--amount=-1",
            "--from",
            "test",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains("amount must not be negative"));
}

#[tokio::test]
async fn transfer_rejects_muxed_source_with_clear_error() {
    let sandbox = &TestEnv::new();
    let recipient = new_account(sandbox, "recipient");

    deploy_sac(sandbox, "native", "test");

    // Muxed (M…) source accounts aren't supported by the invoke pipeline yet
    // (see #2645). Until then the command must reject them up front with a clear
    // message rather than a raw strkey decode error deep in the pipeline.
    let muxed = "MA3D5KRYM6CB7OWQ6TWYRR3Z4T7GNZLKERYNZGGA5SOAOPIFY6YQGAAAAAAAAAPCICBKU";
    sandbox
        .new_assert_cmd("token")
        .args([
            "transfer", "--id", "native", "--to", &recipient, "--amount", "1", "--from", muxed,
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "muxed (M…) source accounts are not yet supported",
        ));
}

#[tokio::test]
async fn transfer_json_failure_returns_error_envelope_on_stdout() {
    let sandbox = &TestEnv::new();
    let test = test_address(sandbox);
    let issuer = new_account(sandbox, "issuer");
    let recipient = new_account(sandbox, "recipient");
    let asset = format!("USDC:{issuer}");

    // Same missing-recipient-trustline failure as above, but in JSON mode: the
    // failure must still surface as a parseable `{ "error": … }` envelope on
    // stdout, and the trustline diagnostic must survive into that message
    // (quiet only suppresses status logging, not the error itself).
    add_trustline(sandbox, "test", &asset);
    issuer_pays(sandbox, "issuer", &test, &asset, 1_000);
    deploy_sac(sandbox, &asset, "issuer");

    let stdout = sandbox
        .new_assert_cmd("token")
        .args([
            "transfer", "--id", &asset, "--to", &recipient, "--amount", "1", "--from", "test",
            "--output", "json",
        ])
        .assert()
        .failure()
        .stdout_as_str();

    let value: Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("stdout should be valid JSON, got: {stdout:?} ({e})"));
    let message = value["error"]["message"]
        .as_str()
        .unwrap_or_else(|| panic!("expected an error envelope with a message, got: {value}"));
    assert!(
        message.contains("trustline entry is missing"),
        "expected the trustline diagnostic in the JSON error message, got: {message}"
    );
}
