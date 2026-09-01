use soroban_cli::tx::ONE_XLM;
use soroban_cli::xdr::{Limits, ReadXdr, TransactionEnvelope};

use soroban_test::{AssertExt, TestEnv};

use crate::integration::util::{gen_account_no_fund, setup_accounts};

#[tokio::test]
async fn payment_with_alias() {
    let sandbox = &TestEnv::new();
    let client = sandbox.client();
    let (test, test1) = setup_accounts(sandbox);
    let test_account = client.get_account(&test).await.unwrap();
    println!("test account has a balance of {}", test_account.balance);

    let before = client.get_account(&test).await.unwrap();
    let test1_account_entry_before = client.get_account(&test1).await.unwrap();

    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "payment",
            "--destination",
            "test1",
            "--amount",
            ONE_XLM.to_string().as_str(),
        ])
        .assert()
        .success();
    let test1_account_entry = client.get_account(&test1).await.unwrap();
    assert_eq!(
        ONE_XLM,
        test1_account_entry.balance - test1_account_entry_before.balance,
        "Should have One XLM more"
    );
    let after = client.get_account(&test).await.unwrap();
    assert_eq!(before.balance - 10_000_100, after.balance);
}

#[tokio::test]
async fn payment() {
    let sandbox = &TestEnv::new();
    let client = sandbox.network.rpc_client().unwrap();
    let (test, test1) = setup_accounts(sandbox);
    let test_account = client.get_account(&test).await.unwrap();
    println!("test account has a balance of {}", test_account.balance);

    let before = client.get_account(&test).await.unwrap();
    let test1_account_entry_before = client.get_account(&test1).await.unwrap();

    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "payment",
            "--destination",
            test1.as_str(),
            "--amount",
            "10_000_000",
        ])
        .assert()
        .success();
    let test1_account_entry = client.get_account(&test1).await.unwrap();
    assert_eq!(
        ONE_XLM,
        test1_account_entry.balance - test1_account_entry_before.balance,
        "Should have One XLM more"
    );
    let after = client.get_account(&test).await.unwrap();
    assert_eq!(before.balance - 10_000_100, after.balance);
}

// A payment to an account that does not exist on the network passes signing
// and submission, then fails at apply time (op_no_destination). That failure
// must preserve the signed envelope in the action log with resubmit
// instructions pinned to the network the send failed on, and `--no-cache`
// must skip the preservation (#2609).
#[test]
fn failed_send_preserves_signed_envelope_in_action_log() {
    let sandbox = &TestEnv::new();
    let missing = gen_account_no_fund(sandbox, "missing");

    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "payment",
            "--destination",
            missing.as_str(),
            "--amount",
            "1",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains("signed envelope was saved"))
        .stderr(predicates::str::contains("stellar tx send --rpc-url"))
        .stderr(predicates::str::contains(sandbox.network.rpc_url.as_str()))
        .stderr(predicates::str::contains("--network-passphrase"));

    let ls = sandbox
        .new_assert_cmd("cache")
        .args(["actionlog", "ls", "--long"])
        .assert()
        .success()
        .stdout_as_str();
    assert_eq!(
        ls.matches("SendFail").count(),
        1,
        "expected exactly one SendFail entry: {ls}"
    );
    let id = ls
        .lines()
        .find(|line| line.contains("SendFail"))
        .and_then(|line| line.split_whitespace().next())
        .expect("the SendFail entry should start with its id")
        .to_string();

    let entry = sandbox
        .new_assert_cmd("cache")
        .args(["actionlog", "read", "--id", &id])
        .assert()
        .success()
        .stdout_as_str();
    let entry: serde_json::Value = serde_json::from_str(&entry).unwrap();
    let envelope_xdr = entry["action"]["send_failed"]["envelope_xdr"]
        .as_str()
        .expect("the send_failed entry should carry the envelope XDR");
    let envelope = TransactionEnvelope::from_xdr_base64(envelope_xdr, Limits::none())
        .expect("the saved envelope must be valid base64 XDR");
    let TransactionEnvelope::Tx(tx_env) = envelope else {
        panic!("expected a signed v1 envelope");
    };
    assert!(
        !tx_env.signatures.is_empty(),
        "the preserved envelope must keep its signatures"
    );

    // With --no-cache the failed send must not be preserved.
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "payment",
            "--no-cache",
            "--destination",
            missing.as_str(),
            "--amount",
            "1",
        ])
        .assert()
        .failure();
    let ls = sandbox
        .new_assert_cmd("cache")
        .args(["actionlog", "ls", "--long"])
        .assert()
        .success()
        .stdout_as_str();
    assert_eq!(
        ls.matches("SendFail").count(),
        1,
        "--no-cache must not add a SendFail entry: {ls}"
    );
}
