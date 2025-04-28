use predicates::prelude::predicate;
use soroban_test::AssertExt;
use soroban_test::TestEnv;
use soroban_rpc::{GetLedgerEntriesResponse, FullLedgerEntries};
use soroban_cli::xdr::{LedgerKey, LedgerKeyAccount, WriteXdr, Limits, PublicKey, Uint256, AccountId, AccountEntry, LedgerEntryData};
use stellar_strkey::ed25519::PublicKey as StrkeyPublicKeyEd25519;

fn new_account(sandbox: &TestEnv, name: &str) -> String {
    sandbox.generate_account(name, None).assert().success();
    sandbox.fund_account(name).success();
    
    sandbox
        .new_assert_cmd("keys")
        .args(["address", name])
        .assert()
        .success()
        .stdout_as_str()
}


// todo: test with --asset, --offer, --data-name
#[tokio::test]
async fn test_account_data(){
    let sandbox = &TestEnv::new();
    let new_account_addr = new_account(sandbox, "new_account");
    let output = sandbox
        .new_assert_cmd("ledger")
        .arg("entry")
        .arg("fetch")
        .arg("--network")
        .arg("testnet")
        .arg("--account")
        .arg("new_account")
        .arg("--asset")
        // though xlm does not have nor need a trustline, "xlm" is a valid argument to `--asset`
        // so this test is including it to make sure that the account ledger entry is still included in the output 
        .arg("xlm")
        .arg("--output")
        .arg("json")
        .assert()
        .success()
        .stdout_as_str();

    // create the expected LedgerKeyAccount key
    let strkey = StrkeyPublicKeyEd25519::from_string(&new_account_addr).unwrap().0;
    let uint256 = Uint256(strkey);
    let pk = PublicKey::PublicKeyTypeEd25519(uint256);
    let account_id = AccountId(pk);
    let expected_key = LedgerKey::Account(LedgerKeyAccount { account_id: account_id.clone() });

    let parsed: FullLedgerEntries = serde_json::from_str(&output).expect("Failed to parse JSON");
    assert!(!parsed.entries.is_empty());
    assert_eq!(parsed.entries[0].key, expected_key);
    if let LedgerEntryData::Account(account) = &parsed.entries[0].val {
        assert_eq!(account.account_id, account_id);
    }
}
