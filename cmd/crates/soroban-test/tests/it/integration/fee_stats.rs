use soroban_rpc::GetFeeStatsResponse;
use soroban_test::{AssertExt, TestEnv};

#[tokio::test]
async fn fee_stats_text_output() {
    let sandbox = &TestEnv::new();
    sandbox
        .new_assert_cmd("fees")
        .arg("stats")
        .arg("--output")
        .arg("text")
        .assert()
        .success()
        .stdout(predicates::str::contains("Max Soroban Inclusion Fee:"))
        .stdout(predicates::str::contains("Max Inclusion Fee:"))
        .stdout(predicates::str::contains("Latest Ledger:"));
}

#[tokio::test]
async fn fee_stats_json_output() {
    let sandbox = &TestEnv::new();
    let output = sandbox
        .new_assert_cmd("fees")
        .arg("stats")
        .arg("--output")
        .arg("json")
        .assert()
        .success()
        .stdout_as_str();
    let fee_stats_response: GetFeeStatsResponse = serde_json::from_str(&output).unwrap();
    assert!(matches!(fee_stats_response, GetFeeStatsResponse { .. }))
}
