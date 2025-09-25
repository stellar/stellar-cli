use crate::integration::util::{setup_accounts, test_address};
use soroban_cli::xdr::{self, ReadXdr};
use soroban_test::{AssertExt, TestEnv};

#[tokio::test]
async fn set_options_add_signer() {
    let sandbox = &TestEnv::new();
    let client = sandbox.network.rpc_client().unwrap();
    let (test, test1) = setup_accounts(sandbox);
    let before = client.get_account(&test).await.unwrap();
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "set-options",
            "--signer",
            test1.as_str(),
            "--signer-weight",
            "1",
        ])
        .assert()
        .success();
    let after = client.get_account(&test).await.unwrap();
    assert_eq!(before.signers.len() + 1, after.signers.len());
    assert_eq!(after.signers.first().unwrap().key, test1.parse().unwrap());
    let key = xdr::LedgerKey::Account(xdr::LedgerKeyAccount {
        account_id: test.parse().unwrap(),
    });
    let res = client.get_ledger_entries(&[key]).await.unwrap();
    let xdr_str = res.entries.unwrap().clone().first().unwrap().clone().xdr;
    let entry = xdr::LedgerEntryData::from_xdr_base64(&xdr_str, xdr::Limits::none()).unwrap();
    let xdr::LedgerEntryData::Account(xdr::AccountEntry { signers, .. }) = entry else {
        panic!();
    };
    assert_eq!(signers.first().unwrap().key, test1.parse().unwrap());

    // Now remove signer with a weight of 0
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "set-options",
            "--signer",
            test1.as_str(),
            "--signer-weight",
            "0",
        ])
        .assert()
        .success();
    let after = client.get_account(&test).await.unwrap();
    assert_eq!(before.signers.len(), after.signers.len());
}

fn build_and_run(sandbox: &TestEnv, cmd: &str, args: &[&str]) -> String {
    let mut args_2 = args.to_vec();
    args_2.push("--build-only");
    let res = sandbox
        .new_assert_cmd(cmd)
        .args(args_2)
        .assert()
        .success()
        .stdout_as_str();
    sandbox.new_assert_cmd(cmd).args(args).assert().success();
    res
}

#[tokio::test]
async fn set_options() {
    let sandbox = &TestEnv::new();
    let client = sandbox.network.rpc_client().unwrap();
    let (test, alice) = setup_accounts(sandbox);
    let before = client.get_account(&test).await.unwrap();
    assert!(before.inflation_dest.is_none());
    let tx_xdr = build_and_run(
        sandbox,
        "tx",
        &[
            "new",
            "set-options",
            "--inflation-dest",
            alice.as_str(),
            "--home-domain",
            "test.com",
            "--master-weight=100",
            "--med-threshold=100",
            "--low-threshold=100",
            "--high-threshold=100",
            "--signer",
            alice.as_str(),
            "--signer-weight=100",
            "--set-required",
            "--set-revocable",
            "--set-clawback-enabled",
            "--set-immutable",
        ],
    );
    println!("{tx_xdr}");
    let after = client.get_account(&test).await.unwrap();
    println!("{before:#?}\n{after:#?}");
    assert_eq!(
        after.flags,
        xdr::AccountFlags::ClawbackEnabledFlag as u32
            | xdr::AccountFlags::ImmutableFlag as u32
            | xdr::AccountFlags::RevocableFlag as u32
            | xdr::AccountFlags::RequiredFlag as u32
    );
    assert_eq!([100, 100, 100, 100], after.thresholds.0);
    assert_eq!(100, after.signers[0].weight);
    assert_eq!(alice, after.signers[0].key.to_string());
    let xdr::PublicKey::PublicKeyTypeEd25519(xdr::Uint256(key)) = after.inflation_dest.unwrap().0;
    assert_eq!(alice, stellar_strkey::ed25519::PublicKey(key).to_string());
    assert_eq!("test.com", after.home_domain.to_string());
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "set-options",
            "--inflation-dest",
            test.as_str(),
            "--home-domain",
            "test.com",
            "--master-weight=100",
            "--med-threshold=100",
            "--low-threshold=100",
            "--high-threshold=100",
            "--signer",
            alice.as_str(),
            "--signer-weight=100",
            "--set-required",
            "--set-revocable",
            "--set-clawback-enabled",
        ])
        .assert()
        .failure();
}

#[tokio::test]
async fn set_some_options() {
    let sandbox = &TestEnv::new();
    let client = sandbox.network.rpc_client().unwrap();
    let test = test_address(sandbox);
    let before = client.get_account(&test).await.unwrap();
    assert!(before.inflation_dest.is_none());
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "set-options",
            "--set-clawback-enabled",
            "--master-weight=100",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains("AuthRevocableRequired"));
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "set-options",
            "--set-revocable",
            "--master-weight=100",
        ])
        .assert()
        .success();
    let after = client.get_account(&test).await.unwrap();
    assert_eq!(after.flags, xdr::AccountFlags::RevocableFlag as u32);
    assert_eq!([100, 0, 0, 0], after.thresholds.0);
    assert!(after.inflation_dest.is_none());
    assert_eq!(
        after.home_domain,
        "".parse::<xdr::StringM<32>>().unwrap().into()
    );
    assert!(after.signers.is_empty());
    sandbox
        .new_assert_cmd("tx")
        .args(["new", "set-options", "--set-clawback-enabled"])
        .assert()
        .success();
    let after = client.get_account(&test).await.unwrap();
    assert_eq!(
        after.flags,
        xdr::AccountFlags::RevocableFlag as u32 | xdr::AccountFlags::ClawbackEnabledFlag as u32
    );
    sandbox
        .new_assert_cmd("tx")
        .args(["new", "set-options", "--clear-clawback-enabled"])
        .assert()
        .success();
    let after = client.get_account(&test).await.unwrap();
    assert_eq!(after.flags, xdr::AccountFlags::RevocableFlag as u32);
    sandbox
        .new_assert_cmd("tx")
        .args(["new", "set-options", "--clear-revocable"])
        .assert()
        .success();
    let after = client.get_account(&test).await.unwrap();
    assert_eq!(after.flags, 0);
    sandbox
        .new_assert_cmd("tx")
        .args(["new", "set-options", "--set-required"])
        .assert()
        .success();
    let after = client.get_account(&test).await.unwrap();
    assert_eq!(after.flags, xdr::AccountFlags::RequiredFlag as u32);
    sandbox
        .new_assert_cmd("tx")
        .args(["new", "set-options", "--clear-required"])
        .assert()
        .success();
    let after = client.get_account(&test).await.unwrap();
    assert_eq!(after.flags, 0);
}
