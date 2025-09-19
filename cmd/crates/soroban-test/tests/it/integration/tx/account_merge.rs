use soroban_test::TestEnv;

use crate::integration::util::setup_accounts;

#[tokio::test]
async fn account_merge() {
    let sandbox = &TestEnv::new();
    let client = sandbox.network.rpc_client().unwrap();
    let (test, test1) = setup_accounts(sandbox);
    let before = client.get_account(&test).await.unwrap();
    let before1 = client.get_account(&test1).await.unwrap();
    let fee = 100;
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "account-merge",
            "--source",
            "test1",
            "--account",
            test.as_str(),
            "--fee",
            fee.to_string().as_str(),
        ])
        .assert()
        .success();
    let after = client.get_account(&test).await.unwrap();
    assert!(client.get_account(&test1).await.is_err());
    assert_eq!(before.balance + before1.balance - fee, after.balance);
}

#[tokio::test]
async fn account_merge_with_alias() {
    let sandbox = &TestEnv::new();
    let client = sandbox.client();
    let (test, test1) = setup_accounts(sandbox);
    let before = client.get_account(&test).await.unwrap();
    let before1 = client.get_account(&test1).await.unwrap();
    let fee = 100;
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "account-merge",
            "--source",
            "test1",
            "--account",
            "test",
            "--fee",
            fee.to_string().as_str(),
        ])
        .assert()
        .success();
    let after = client.get_account(&test).await.unwrap();
    assert!(client.get_account(&test1).await.is_err());
    assert_eq!(before.balance + before1.balance - fee, after.balance);
}
