use soroban_cli::{config::locator, tx::builder, utils::contract_id_hash_from_asset};

use soroban_test::TestEnv;

use crate::integration::util::{issue_asset, new_account, setup_accounts};

#[tokio::test]
async fn change_trust() {
    let sandbox = &TestEnv::new();
    let (test, issuer) = setup_accounts(sandbox);
    let asset = &format!("usdc:{issuer}");

    let limit = 100_000_000;
    let half_limit = limit / 2;
    issue_asset(sandbox, &test, asset, limit, half_limit).await;
    sandbox
        .new_assert_cmd("contract")
        .arg("asset")
        .arg("deploy")
        .arg("--asset")
        .arg(asset)
        .assert()
        .success();

    let id = contract_id_hash_from_asset(
        &asset
            .parse::<builder::Asset>()
            .unwrap()
            .resolve(&locator::Args::default())
            .unwrap(),
        &sandbox.network.network_passphrase,
    );
    sandbox
        .new_assert_cmd("contract")
        .args([
            "invoke",
            "--id",
            &id.to_string(),
            "--",
            "balance",
            "--id",
            &test,
        ])
        .assert()
        .stdout(format!("\"{half_limit}\"\n"));

    let bob = new_account(sandbox, "bob");
    let bobs_limit = half_limit / 2;
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "change-trust",
            "--source=bob",
            "--line",
            asset,
            "--limit",
            bobs_limit.to_string().as_str(),
        ])
        .assert()
        .success();
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "payment",
            "--destination",
            &bob,
            "--asset",
            asset,
            "--amount",
            half_limit.to_string().as_str(),
        ])
        .assert()
        .failure();
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "payment",
            "--destination",
            &bob,
            "--asset",
            asset,
            "--amount",
            bobs_limit.to_string().as_str(),
        ])
        .assert()
        .success();
    sandbox
        .new_assert_cmd("contract")
        .args([
            "invoke",
            "--id",
            &id.to_string(),
            "--",
            "balance",
            "--id",
            &bob,
        ])
        .assert()
        .stdout(format!("\"{bobs_limit}\"\n"));
}

#[tokio::test]
async fn set_trustline_flags() {
    let sandbox = &TestEnv::new();
    let (test, test1_address) = setup_accounts(sandbox);
    let asset = "usdc:test1";
    issue_asset(sandbox, &test, asset, 100_000, 100).await;
    sandbox
        .new_assert_cmd("contract")
        .arg("asset")
        .arg("deploy")
        .arg("--asset")
        .arg(asset)
        .assert()
        .success();
    let id = contract_id_hash_from_asset(
        &format!("usdc:{test1_address}")
            .parse::<builder::Asset>()
            .unwrap()
            .resolve(&locator::Args::default())
            .unwrap(),
        &sandbox.network.network_passphrase,
    );

    sandbox
        .new_assert_cmd("contract")
        .args([
            "invoke",
            "--id",
            &id.to_string(),
            "--",
            "authorized",
            "--id",
            &test,
        ])
        .assert()
        .success()
        .stdout("true\n");
}
