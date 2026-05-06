use soroban_rpc::{GetLatestLedgerResponse, GetLedgersResponse};
use soroban_test::{AssertExt, TestEnv};
mod entry;

#[tokio::test]
async fn ledger_latest() {
    let sandbox = &TestEnv::new();
    sandbox
        .new_assert_cmd("ledger")
        .arg("latest")
        .arg("--network")
        .arg("testnet")
        .assert()
        .success()
        .stdout(predicates::str::contains("Sequence:"))
        .stdout(predicates::str::contains("Protocol Version:"))
        .stdout(predicates::str::contains("Hash:"));
}

#[tokio::test]
async fn ledger_fetch() {
    let sandbox = &TestEnv::new();
    let latest_ledger_response = sandbox
        .new_assert_cmd("ledger")
        .arg("latest")
        .arg("--output")
        .arg("json")
        .assert()
        .success()
        .stdout_as_str();

    let latest_ledger: GetLatestLedgerResponse =
        serde_json::from_str(&latest_ledger_response).unwrap();
    let _latest_ledger_seq = latest_ledger.sequence;
    let ledger_to_fetch = latest_ledger.sequence - 1;

    let ledger_limit = 2;
    let ledger_response = sandbox
        .new_assert_cmd("ledger")
        .arg("fetch")
        .arg(ledger_to_fetch.to_string())
        .arg("--output")
        .arg("json")
        .arg("--limit")
        .arg(ledger_limit.to_string())
        .assert()
        .success()
        .stdout_as_str();

    let ledger: GetLedgersResponse = serde_json::from_str(&ledger_response).unwrap();
    assert!(matches!(
        ledger,
        GetLedgersResponse {
            latest_ledger: _latest_ledger_seq,
            ..
        }
    ));
    assert_eq!(ledger.ledgers.len(), ledger_limit);
}

#[tokio::test]
async fn ledger_fetch_xdr_fields() {
    let sandbox = &TestEnv::new();
    let latest_ledger_response = sandbox
        .new_assert_cmd("ledger")
        .arg("latest")
        .arg("--output")
        .arg("json")
        .assert()
        .success()
        .stdout_as_str();

    let latest_ledger: GetLatestLedgerResponse =
        serde_json::from_str(&latest_ledger_response).unwrap();
    let latest_ledger_seq = latest_ledger.sequence;

    // when xdr-format is json, the headerXdr and metadataXdr fields are empty strings
    let ledger_response = sandbox
        .new_assert_cmd("ledger")
        .arg("fetch")
        .arg(latest_ledger_seq.to_string())
        .arg("--output")
        .arg("json")
        .arg("--xdr-format")
        .arg("json")
        .assert()
        .success()
        .stdout_as_str();

    let ledger: GetLedgersResponse = serde_json::from_str(&ledger_response).unwrap();
    assert_eq!(ledger.ledgers[0].header_xdr, "");
    assert_eq!(ledger.ledgers[0].metadata_xdr, "");

    // when xdr-format is xdr, the headerJson and metadataJson fields are null
    let ledger_response = sandbox
        .new_assert_cmd("ledger")
        .arg("fetch")
        .arg(latest_ledger_seq.to_string())
        .arg("--output")
        .arg("json")
        .arg("--xdr-format")
        .arg("xdr")
        .assert()
        .success()
        .stdout_as_str();

    let ledger: GetLedgersResponse = serde_json::from_str(&ledger_response).unwrap();
    assert!(ledger.ledgers[0].header_json.is_none());
    assert!(ledger.ledgers[0].metadata_json.is_none());
}
