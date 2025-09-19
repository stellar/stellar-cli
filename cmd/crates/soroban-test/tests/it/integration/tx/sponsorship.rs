use soroban_test::{AssertExt, TestEnv};

use crate::integration::util::{
    gen_account_no_fund, get_sponsoring_count, new_account, test_address,
};

#[tokio::test]
async fn begin_sponsoring_future_reserves() {
    let sandbox = &TestEnv::new();
    let client = sandbox.network.rpc_client().unwrap();

    // Create sponsor account (use test account as sponsor)
    let sponsor = test_address(sandbox);

    // Create a new account to sponsor (but don't fund it)
    let sponsored_account = gen_account_no_fund(sandbox, "sponsored");

    let sponsor_balance_before = client.get_account(&sponsor).await.unwrap().balance;

    let sponsor_tx = sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "begin-sponsoring-future-reserves",
            "--source-account",
            "test",
            "--sponsored-id",
            &sponsored_account,
            "--fee",
            "1000000", // Higher fee for sponsoring operations
            "--build-only",
        ])
        .assert()
        .success()
        .stdout_as_str();

    // Add create account operation with sponsor as operation source
    let create_account_tx = sandbox
        .new_assert_cmd("tx")
        .args([
            "op",
            "add",
            "create-account",
            "--destination",
            &sponsored_account,
            "--starting-balance",
            "50000000",
            "--operation-source-account",
            "test", // sponsor account
        ])
        .write_stdin(sponsor_tx.as_bytes())
        .assert()
        .success()
        .stdout_as_str();

    // Add end sponsoring future reserves operation with sponsored account as operation source
    let complete_tx = sandbox
        .new_assert_cmd("tx")
        .args([
            "op",
            "add",
            "end-sponsoring-future-reserves",
            "--operation-source-account",
            "sponsored",
        ])
        .write_stdin(create_account_tx.as_bytes())
        .assert()
        .success()
        .stdout_as_str();

    // Sign with sponsor first
    let sponsor_signed_tx = sandbox
        .new_assert_cmd("tx")
        .args(["sign", "--sign-with-key=test"])
        .write_stdin(complete_tx.as_bytes())
        .assert()
        .success()
        .stdout_as_str();

    // Sign with sponsored account second
    let fully_signed_tx = sandbox
        .new_assert_cmd("tx")
        .args(["sign", "--sign-with-key=sponsored"])
        .write_stdin(sponsor_signed_tx.as_bytes())
        .assert()
        .success()
        .stdout_as_str();

    // Submit the transaction
    sandbox
        .new_assert_cmd("tx")
        .arg("send")
        .write_stdin(fully_signed_tx.as_bytes())
        .assert()
        .success();

    let sponsor_balance_after = client.get_account(&sponsor).await.unwrap().balance;

    // The sponsored account should exist
    let sponsored_account_info = client.get_account(&sponsored_account).await.unwrap();
    assert_eq!(sponsored_account_info.balance, 50000000);

    // The sponsor account balance should be lower due to sponsoring the reserves
    assert!(
        sponsor_balance_after < sponsor_balance_before,
        "Sponsor account should have paid for the sponsored account reserves"
    );
}

#[tokio::test]
async fn revoke_sponsorship_account() {
    let sandbox = &TestEnv::new();
    let client = sandbox.network.rpc_client().unwrap();

    // Create sponsor account (use test account as sponsor)
    let _sponsor = test_address(sandbox);

    // Create a new account to sponsor (but don't fund it)
    let sponsored_account = gen_account_no_fund(sandbox, "sponsored");

    // Set up sponsorship first
    let sponsor_tx = sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "begin-sponsoring-future-reserves",
            "--source-account",
            "test",
            "--sponsored-id",
            &sponsored_account,
            "--fee",
            "1000000",
            "--build-only",
        ])
        .assert()
        .success()
        .stdout_as_str();

    // Add create account operation with sponsor as operation source
    let create_account_tx = sandbox
        .new_assert_cmd("tx")
        .args([
            "op",
            "add",
            "create-account",
            "--destination",
            &sponsored_account,
            "--starting-balance",
            "50000000",
            "--operation-source-account",
            "test",
        ])
        .write_stdin(sponsor_tx.as_bytes())
        .assert()
        .success()
        .stdout_as_str();

    // Add end sponsoring future reserves operation
    let complete_tx = sandbox
        .new_assert_cmd("tx")
        .args([
            "op",
            "add",
            "end-sponsoring-future-reserves",
            "--operation-source-account",
            "sponsored",
        ])
        .write_stdin(create_account_tx.as_bytes())
        .assert()
        .success()
        .stdout_as_str();

    // Sign with sponsor first
    let sponsor_signed_tx = sandbox
        .new_assert_cmd("tx")
        .args(["sign", "--sign-with-key=test"])
        .write_stdin(complete_tx.as_bytes())
        .assert()
        .success()
        .stdout_as_str();

    // Sign with sponsored account second
    let fully_signed_tx = sandbox
        .new_assert_cmd("tx")
        .args(["sign", "--sign-with-key=sponsored"])
        .write_stdin(sponsor_signed_tx.as_bytes())
        .assert()
        .success()
        .stdout_as_str();

    // Submit the sponsorship transaction
    sandbox
        .new_assert_cmd("tx")
        .arg("send")
        .write_stdin(fully_signed_tx.as_bytes())
        .assert()
        .success();

    // Verify the sponsored account exists and is sponsored
    let sponsored_account_info = client.get_account(&sponsored_account).await.unwrap();
    assert_eq!(sponsored_account_info.balance, 50000000);

    // Check sponsor's sponsoring count before revoking
    let sponsor_account_before = client.get_account(&test_address(sandbox)).await.unwrap();
    let num_sponsoring_before = get_sponsoring_count(&sponsor_account_before);

    // Now test revoke sponsorship for the account ledger entry
    // The sponsor should be able to revoke sponsorship of the account
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "revoke-sponsorship",
            "--source-account",
            "test", // sponsor account
            "--account-id",
            &sponsored_account,
        ])
        .assert()
        .success();

    // Verify that the sponsorship was revoked by checking the sponsor's sponsoring count
    let sponsor_account_after = client.get_account(&test_address(sandbox)).await.unwrap();
    let num_sponsoring_after = get_sponsoring_count(&sponsor_account_after);

    // The sponsor should have fewer sponsored entries after revoking sponsorship
    assert!(
        num_sponsoring_after < num_sponsoring_before,
        "Sponsor should have fewer sponsored entries after revoking sponsorship. Before: {}, After: {}",
        num_sponsoring_before,
        num_sponsoring_after
    );
}

#[tokio::test]
async fn revoke_sponsorship_trustline() {
    let sandbox = &TestEnv::new();

    let sponsored_account = new_account(sandbox, "sponsored");
    let _issuer_account = new_account(sandbox, "issuer");
    let asset = "USD:issuer".to_string();

    let tx = sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "begin-sponsoring-future-reserves",
            "--source-account",
            "test",
            "--sponsored-id",
            &sponsored_account,
            "--fee",
            "1000000",
            "--build-only",
        ])
        .assert()
        .success()
        .stdout_as_str();

    let tx = sandbox
        .new_assert_cmd("tx")
        .args([
            "op",
            "add",
            "change-trust",
            "--operation-source-account",
            "sponsored",
            "--line",
            &asset,
        ])
        .write_stdin(tx.as_bytes())
        .assert()
        .success()
        .stdout_as_str();

    let tx = sandbox
        .new_assert_cmd("tx")
        .args([
            "op",
            "add",
            "end-sponsoring-future-reserves",
            "--operation-source-account",
            "sponsored",
        ])
        .write_stdin(tx.as_bytes())
        .assert()
        .success()
        .stdout_as_str();

    let tx_signed = sandbox
        .new_assert_cmd("tx")
        .args(["sign", "--sign-with-key=test"])
        .write_stdin(tx.as_bytes())
        .assert()
        .success()
        .stdout_as_str();

    let tx_signed = sandbox
        .new_assert_cmd("tx")
        .args(["sign", "--sign-with-key=sponsored"])
        .write_stdin(tx_signed.as_bytes())
        .assert()
        .success()
        .stdout_as_str();

    sandbox
        .new_assert_cmd("tx")
        .arg("send")
        .write_stdin(tx_signed.as_bytes())
        .assert()
        .success();

    // Check if trustline was created and sponsorship is working
    let client = sandbox.network.rpc_client().unwrap();
    let sponsored_account_details = client.get_account(&sponsored_account).await.unwrap();
    println!(
        "Sponsored account sub-entries: {}",
        sponsored_account_details.num_sub_entries
    );

    let test_address = test_address(sandbox);
    let sponsor_account_details = client.get_account(&test_address).await.unwrap();
    let sponsoring_count = get_sponsoring_count(&sponsor_account_details);
    println!("Sponsor account sponsoring count: {}", sponsoring_count);
    println!("Sponsored account address: {}", sponsored_account);
    println!("Test/sponsor account address: {}", test_address);
    println!("Asset: {}", asset);

    // Get current sponsoring count for comparison before revoke
    let sponsoring_count_before = sponsoring_count;

    // Test revoke trustline sponsorship
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "revoke-sponsorship",
            "--source-account",
            "test",
            "--account-id",
            &sponsored_account,
            "--asset",
            &asset,
        ])
        .assert()
        .success();

    // Verify sponsorship was revoked by checking sponsoring count decreased
    let account_details_after = client.get_account(&test_address).await.unwrap();
    let sponsoring_count_after = get_sponsoring_count(&account_details_after);
    assert!(
        sponsoring_count_after < sponsoring_count_before,
        "Sponsor should have fewer sponsored entries after revoking sponsorship. Before: {}, After: {}",
        sponsoring_count_before,
        sponsoring_count_after
    );
}

#[tokio::test]
async fn revoke_sponsorship_data() {
    let sandbox = &TestEnv::new();

    let sponsored_account = new_account(sandbox, "sponsored");
    let _issuer_account = new_account(sandbox, "issuer");
    let asset = "USD:issuer".to_string();

    let tx = sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "begin-sponsoring-future-reserves",
            "--source-account",
            "test",
            "--sponsored-id",
            &sponsored_account,
            "--fee",
            "1000000",
            "--build-only",
        ])
        .assert()
        .success()
        .stdout_as_str();

    let tx = sandbox
        .new_assert_cmd("tx")
        .args([
            "op",
            "add",
            "manage-data",
            "--data-name",
            "msg",
            "--data-value",
            "beefface",
            "--operation-source-account",
            "sponsored",
        ])
        .write_stdin(tx.as_bytes())
        .assert()
        .success()
        .stdout_as_str();

    let tx = sandbox
        .new_assert_cmd("tx")
        .args([
            "op",
            "add",
            "end-sponsoring-future-reserves",
            "--operation-source-account",
            "sponsored",
        ])
        .write_stdin(tx.as_bytes())
        .assert()
        .success()
        .stdout_as_str();

    let tx_signed = sandbox
        .new_assert_cmd("tx")
        .args(["sign", "--sign-with-key=test"])
        .write_stdin(tx.as_bytes())
        .assert()
        .success()
        .stdout_as_str();

    let tx_signed = sandbox
        .new_assert_cmd("tx")
        .args(["sign", "--sign-with-key=sponsored"])
        .write_stdin(tx_signed.as_bytes())
        .assert()
        .success()
        .stdout_as_str();

    sandbox
        .new_assert_cmd("tx")
        .arg("send")
        .write_stdin(tx_signed.as_bytes())
        .assert()
        .success();

    let client = sandbox.network.rpc_client().unwrap();
    let sponsored_account_details = client.get_account(&sponsored_account).await.unwrap();
    println!(
        "Sponsored account sub-entries: {}",
        sponsored_account_details.num_sub_entries
    );

    let test_address = test_address(sandbox);
    let sponsor_account_details = client.get_account(&test_address).await.unwrap();
    let sponsoring_count = get_sponsoring_count(&sponsor_account_details);
    println!("Sponsor account sponsoring count: {}", sponsoring_count);
    println!("Sponsored account address: {}", sponsored_account);
    println!("Test/sponsor account address: {}", test_address);
    println!("Asset: {}", asset);

    // Get current sponsoring count for comparison before revoke
    let sponsoring_count_before = sponsoring_count;

    // Test revoke manage data sponsorship
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "revoke-sponsorship",
            "--source-account",
            "test",
            "--account-id",
            &sponsored_account,
            "--data-name",
            "msg",
        ])
        .assert()
        .success();

    // Verify sponsorship was revoked by checking sponsoring count decreased
    let account_details_after = client.get_account(&test_address).await.unwrap();
    let sponsoring_count_after = get_sponsoring_count(&account_details_after);
    assert!(
        sponsoring_count_after < sponsoring_count_before,
        "Sponsor should have fewer sponsored entries after revoking sponsorship. Before: {}, After: {}",
        sponsoring_count_before,
        sponsoring_count_after
    );
}

#[tokio::test]
async fn revoke_sponsorship_signer() {
    let sandbox = &TestEnv::new();

    let sponsored_account = new_account(sandbox, "sponsored");

    // Generate a new signer account without funding
    let signer_account = gen_account_no_fund(sandbox, "signer");

    let tx = sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "begin-sponsoring-future-reserves",
            "--source-account",
            "test",
            "--sponsored-id",
            &sponsored_account,
            "--fee",
            "1000000",
            "--build-only",
        ])
        .assert()
        .success()
        .stdout_as_str();

    let tx = sandbox
        .new_assert_cmd("tx")
        .args([
            "op",
            "add",
            "set-options",
            "--signer",
            &signer_account,
            "--signer-weight",
            "1",
            "--operation-source-account",
            "sponsored",
        ])
        .write_stdin(tx.as_bytes())
        .assert()
        .success()
        .stdout_as_str();

    let tx = sandbox
        .new_assert_cmd("tx")
        .args([
            "op",
            "add",
            "end-sponsoring-future-reserves",
            "--operation-source-account",
            "sponsored",
        ])
        .write_stdin(tx.as_bytes())
        .assert()
        .success()
        .stdout_as_str();

    let tx_signed = sandbox
        .new_assert_cmd("tx")
        .args(["sign", "--sign-with-key=test"])
        .write_stdin(tx.as_bytes())
        .assert()
        .success()
        .stdout_as_str();

    let tx_signed = sandbox
        .new_assert_cmd("tx")
        .args(["sign", "--sign-with-key=sponsored"])
        .write_stdin(tx_signed.as_bytes())
        .assert()
        .success()
        .stdout_as_str();

    sandbox
        .new_assert_cmd("tx")
        .arg("send")
        .write_stdin(tx_signed.as_bytes())
        .assert()
        .success();

    let client = sandbox.network.rpc_client().unwrap();
    let sponsored_account_details = client.get_account(&sponsored_account).await.unwrap();
    println!(
        "Sponsored account sub-entries: {}",
        sponsored_account_details.num_sub_entries
    );

    let test_address = test_address(sandbox);
    let sponsor_account_details = client.get_account(&test_address).await.unwrap();
    let sponsoring_count = get_sponsoring_count(&sponsor_account_details);
    println!("Sponsor account sponsoring count: {}", sponsoring_count);
    println!("Sponsored account address: {}", sponsored_account);
    println!("Test/sponsor account address: {}", test_address);
    println!("Signer account: {}", signer_account);

    // Get current sponsoring count for comparison before revoke
    let sponsoring_count_before = sponsoring_count;

    // Test revoke signer sponsorship
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "revoke-sponsorship",
            "--source-account",
            "test",
            "--account-id",
            &sponsored_account,
            "--signer-key",
            &signer_account,
        ])
        .assert()
        .success();

    // Verify sponsorship was revoked by checking sponsoring count decreased
    let account_details_after = client.get_account(&test_address).await.unwrap();
    let sponsoring_count_after = get_sponsoring_count(&account_details_after);
    assert!(
        sponsoring_count_after < sponsoring_count_before,
        "Sponsor should have fewer sponsored entries after revoking sponsorship. Before: {}, After: {}",
        sponsoring_count_before,
        sponsoring_count_after
    );
}

#[tokio::test]
async fn revoke_sponsorship_offer() {
    let sandbox = &TestEnv::new();

    let sponsored_account = new_account(sandbox, "sponsored");
    let _issuer_account = new_account(sandbox, "issuer");
    let selling_asset = "USD:issuer".to_string();

    // First create a trustline for the selling asset
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "change-trust",
            "--source-account",
            "sponsored",
            "--line",
            &selling_asset,
        ])
        .assert()
        .success();

    // Fund the sponsored account with some USD tokens to sell
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "payment",
            "--source-account",
            "issuer",
            "--destination",
            "sponsored",
            "--asset",
            &selling_asset,
            "--amount",
            "1000",
        ])
        .assert()
        .success();

    let tx = sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "begin-sponsoring-future-reserves",
            "--source-account",
            "test",
            "--sponsored-id",
            &sponsored_account,
            "--fee",
            "1000000",
            "--build-only",
        ])
        .assert()
        .success()
        .stdout_as_str();

    let tx = sandbox
        .new_assert_cmd("tx")
        .args([
            "op",
            "add",
            "manage-sell-offer",
            "--selling",
            &selling_asset,
            "--buying",
            "native",
            "--amount",
            "100",
            "--price",
            "1:1",
            "--operation-source-account",
            "sponsored",
        ])
        .write_stdin(tx.as_bytes())
        .assert()
        .success()
        .stdout_as_str();

    let tx = sandbox
        .new_assert_cmd("tx")
        .args([
            "op",
            "add",
            "end-sponsoring-future-reserves",
            "--operation-source-account",
            "sponsored",
        ])
        .write_stdin(tx.as_bytes())
        .assert()
        .success()
        .stdout_as_str();

    let tx_signed = sandbox
        .new_assert_cmd("tx")
        .args(["sign", "--sign-with-key=test"])
        .write_stdin(tx.as_bytes())
        .assert()
        .success()
        .stdout_as_str();

    let tx_signed = sandbox
        .new_assert_cmd("tx")
        .args(["sign", "--sign-with-key=sponsored"])
        .write_stdin(tx_signed.as_bytes())
        .assert()
        .success()
        .stdout_as_str();

    sandbox
        .new_assert_cmd("tx")
        .arg("send")
        .write_stdin(tx_signed.as_bytes())
        .assert()
        .success();

    let client = sandbox.network.rpc_client().unwrap();
    let sponsored_account_details = client.get_account(&sponsored_account).await.unwrap();
    println!(
        "Sponsored account sub-entries: {}",
        sponsored_account_details.num_sub_entries
    );

    let test_address = test_address(sandbox);
    let sponsor_account_details = client.get_account(&test_address).await.unwrap();
    let sponsoring_count = get_sponsoring_count(&sponsor_account_details);
    println!("Sponsor account sponsoring count: {}", sponsoring_count);
    println!("Sponsored account address: {}", sponsored_account);
    println!("Test/sponsor account address: {}", test_address);

    // Get current sponsoring count for comparison before revoke
    let sponsoring_count_before = sponsoring_count;

    // Test revoke offer sponsorship - we need the offer ID
    // Fetch the actual offer ID from Horizon
    let horizon_url = format!(
        "http://localhost:8000/accounts/{}/offers",
        sponsored_account
    );
    let response = reqwest::get(&horizon_url).await.unwrap();
    let json: serde_json::Value = response.json().await.unwrap();
    let offers = &json["_embedded"]["records"];
    assert!(
        !offers.as_array().unwrap().is_empty(),
        "No offers found for sponsored account"
    );
    let offer_id = offers[0]["id"].as_str().unwrap();

    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "revoke-sponsorship",
            "--source-account",
            "test",
            "--account-id",
            &sponsored_account,
            "--offer-id",
            offer_id,
        ])
        .assert()
        .success();

    // Verify sponsorship was revoked by checking sponsoring count decreased
    let account_details_after = client.get_account(&test_address).await.unwrap();
    let sponsoring_count_after = get_sponsoring_count(&account_details_after);
    assert!(
        sponsoring_count_after < sponsoring_count_before,
        "Sponsor should have fewer sponsored entries after revoking sponsorship. Before: {}, After: {}",
        sponsoring_count_before,
        sponsoring_count_after
    );
}
