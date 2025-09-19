use crate::integration::util::test_address;
use soroban_cli::xdr::SequenceNumber;
use soroban_test::TestEnv;

#[tokio::test]
async fn bump_sequence() {
    let sandbox = &TestEnv::new();
    let client = sandbox.network.rpc_client().unwrap();
    let test = test_address(sandbox);
    let before = client.get_account(&test).await.unwrap();
    let amount = 50;
    let seq = SequenceNumber(before.seq_num.0 + amount);
    // bump sequence tx new
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "bump-sequence",
            "--bump-to",
            seq.0.to_string().as_str(),
        ])
        .assert()
        .success();
    let after = client.get_account(&test).await.unwrap();
    assert_eq!(seq, after.seq_num);
}
