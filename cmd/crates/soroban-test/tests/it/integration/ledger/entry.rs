use soroban_cli::xdr::{
    AccountId, AlphaNum4, AssetCode4, ConfigSettingId, ContractDataDurability, Hash,
    LedgerEntryData, LedgerKey, LedgerKeyAccount, LedgerKeyConfigSetting, LedgerKeyContractCode,
    LedgerKeyContractData, LedgerKeyData, LedgerKeyTrustLine, Limits, PublicKey, ScAddress, ScVal,
    String64, StringM, TrustLineAsset, Uint256, WriteXdr,
};
use soroban_rpc::FullLedgerEntries;
use soroban_spec_tools::utils::padded_hex_from_str;
use soroban_test::AssertExt;
use soroban_test::TestEnv;
use stellar_strkey::{ed25519::PublicKey as StrkeyPublicKeyEd25519, Contract};

use crate::integration::util::{deploy_contract, test_address, DeployOptions, HELLO_WORLD};

// account data tests
// todo: test with--offer,
#[tokio::test]
async fn ledger_entry_account_only_with_account_alias() {
    let sandbox = &TestEnv::new();
    let account_alias = "new_account";
    let new_account_addr = new_account(sandbox, account_alias);
    let output = sandbox
        .new_assert_cmd("ledger")
        .arg("entry")
        .arg("fetch")
        .arg("account")
        .arg(account_alias)
        .arg("--network")
        .arg("testnet")
        .assert()
        .success()
        .stdout_as_str();

    let (_, expected_key) = expected_account_ledger_key(&new_account_addr).await;
    let parsed: FullLedgerEntries = serde_json::from_str(&output).expect("Failed to parse JSON");

    assert!(!parsed.entries.is_empty());
    assert_eq!(parsed.entries[0].key, expected_key);
    assert!(matches!(
        parsed.entries[0].val,
        LedgerEntryData::Account { .. }
    ));
}

#[tokio::test]
async fn ledger_entry_account_only_with_account_addr() {
    let sandbox = &TestEnv::new();
    let new_account_addr = new_account(sandbox, "new_account");
    let output = sandbox
        .new_assert_cmd("ledger")
        .arg("entry")
        .arg("fetch")
        .arg("account")
        .arg(&new_account_addr)
        .arg("--network")
        .arg("testnet")
        .assert()
        .success()
        .stdout_as_str();

    let (_, expected_key) = expected_account_ledger_key(&new_account_addr).await;
    let parsed: FullLedgerEntries = serde_json::from_str(&output).expect("Failed to parse JSON");

    assert!(!parsed.entries.is_empty());
    assert_eq!(parsed.entries[0].key, expected_key);
    assert!(matches!(
        parsed.entries[0].val,
        LedgerEntryData::Account { .. }
    ));
}

#[tokio::test]
async fn ledger_entry_account_asset_xlm() {
    let sandbox = &TestEnv::new();
    let account_alias = "new_account";
    let new_account_addr = new_account(sandbox, account_alias);
    let output = sandbox
        .new_assert_cmd("ledger")
        .arg("entry")
        .arg("fetch")
        .arg("account")
        .arg(account_alias)
        .arg("--network")
        .arg("testnet")
        .arg("--asset")
        // though xlm does not have, nor need, a trustline, "xlm" is a valid argument to `--asset`
        // this test is including it to make sure that the account ledger entry is still included in the output
        .arg("xlm")
        .assert()
        .success()
        .stdout_as_str();

    let (_, expected_key) = expected_account_ledger_key(&new_account_addr).await;

    let parsed: FullLedgerEntries = serde_json::from_str(&output).expect("Failed to parse JSON");
    assert!(!parsed.entries.is_empty());
    assert_eq!(parsed.entries[0].key, expected_key);
    assert!(matches!(
        parsed.entries[0].val,
        LedgerEntryData::Account { .. }
    ));
}

#[tokio::test]
async fn ledger_entry_account_asset_usdc() {
    let sandbox = &TestEnv::new();
    let test_account_alias = "test";
    let test_account_address = test_address(sandbox);
    let issuer_alias = "test1";
    let issuer_address = new_account(sandbox, issuer_alias);
    let asset = &format!("usdc:{issuer_address}");
    let limit = 100_000;
    let initial_balance = 100;
    issue_asset(
        sandbox,
        &test_account_address,
        asset,
        limit,
        initial_balance,
    )
    .await;

    let output = sandbox
        .new_assert_cmd("ledger")
        .arg("entry")
        .arg("fetch")
        .arg("account")
        .arg(test_account_alias)
        .arg("--network")
        .arg("testnet")
        .arg("--asset")
        .arg(asset)
        .assert()
        .success()
        .stdout_as_str();

    let (account_id, expected_account_key) =
        expected_account_ledger_key(&test_account_address).await;
    let (issuer_account_id, _) = expected_account_ledger_key(&issuer_address).await;

    let trustline_asset = TrustLineAsset::CreditAlphanum4(AlphaNum4 {
        asset_code: AssetCode4(*b"usdc"),
        issuer: issuer_account_id,
    });
    let expected_trustline_key = LedgerKey::Trustline(LedgerKeyTrustLine {
        account_id,
        asset: trustline_asset,
    });

    let parsed: FullLedgerEntries = serde_json::from_str(&output).expect("Failed to parse JSON");
    assert!(!parsed.entries.is_empty());

    let account_entry = &parsed.entries[0];
    assert_eq!(account_entry.key, expected_account_key);
    assert!(matches!(account_entry.val, LedgerEntryData::Account { .. }));

    let trustline_entry = &parsed.entries[1];
    assert_eq!(trustline_entry.key, expected_trustline_key);
    assert!(matches!(
        trustline_entry.val,
        LedgerEntryData::Trustline { .. }
    ));
}

#[tokio::test]
async fn ledger_entry_account_data() {
    let sandbox = &TestEnv::new();
    let account_alias = "new_account";
    let new_account_addr = new_account(sandbox, account_alias);
    let data_name = "test_data_key";
    add_account_data(sandbox, account_alias, data_name, "abcdef").await;

    let output = sandbox
        .new_assert_cmd("ledger")
        .arg("entry")
        .arg("fetch")
        .arg("account")
        .arg(account_alias)
        .arg("--network")
        .arg("testnet")
        .arg("--data-name")
        .arg(data_name)
        .assert()
        .success()
        .stdout_as_str();

    let parsed: FullLedgerEntries = serde_json::from_str(&output).expect("Failed to parse JSON");
    assert!(!parsed.entries.is_empty());

    let (account_id, expected_account_key) = expected_account_ledger_key(&new_account_addr).await;

    let account_entry = &parsed.entries[0];
    assert_eq!(account_entry.key, expected_account_key);
    assert!(matches!(account_entry.val, LedgerEntryData::Account { .. }));

    let data_entry = &parsed.entries[1];
    let name_bounded_string = StringM::<64>::try_from(data_name).unwrap();
    let expected_data_key = LedgerKey::Data(LedgerKeyData {
        account_id,
        data_name: String64::from(name_bounded_string),
    });
    assert_eq!(data_entry.key, expected_data_key);
    assert!(matches!(data_entry.val, LedgerEntryData::Data { .. }));
}

#[tokio::test]
async fn ledger_entries_hide_account() {
    let sandbox = &TestEnv::new();
    let account_alias = "new_account";
    let new_account_addr = new_account(sandbox, account_alias);
    let data_name = "test_data_key";
    add_account_data(sandbox, account_alias, data_name, "abcdef").await;

    let output = sandbox
        .new_assert_cmd("ledger")
        .arg("entry")
        .arg("fetch")
        .arg("account")
        .arg(account_alias)
        .arg("--network")
        .arg("testnet")
        .arg("--hide-account")
        .arg("--data-name")
        .arg(data_name)
        .assert()
        .success()
        .stdout_as_str();

    let parsed: FullLedgerEntries = serde_json::from_str(&output).expect("Failed to parse JSON");
    assert!(!parsed.entries.is_empty());
    assert_eq!(parsed.entries.len(), 1);
    

    let (account_id, _) = expected_account_ledger_key(&new_account_addr).await;

    let data_entry = &parsed.entries[0];
    let name_bounded_string = StringM::<64>::try_from(data_name).unwrap();
    let expected_data_key = LedgerKey::Data(LedgerKeyData {
        account_id,
        data_name: String64::from(name_bounded_string),
    });
    assert_eq!(data_entry.key, expected_data_key);
    assert!(matches!(data_entry.val, LedgerEntryData::Data { .. }));
}

// contract data tests
#[tokio::test]
async fn ledger_entry_contract_data() {
    let sandbox = &TestEnv::new();
    let test_account_alias = "test";
    let contract_id = deploy_contract(
        sandbox,
        HELLO_WORLD,
        DeployOptions {
            deployer: Some(test_account_alias.to_string()),
            ..Default::default()
        },
    )
    .await;

    let storage_key = "COUNTER";
    let storage_key_xdr = ScVal::Symbol(storage_key.try_into().unwrap())
        .to_xdr_base64(Limits::none())
        .unwrap();

    // update contract storage
    sandbox
        .invoke_with_test(&["--id", &contract_id, "--", "inc"])
        .await
        .unwrap();

    // get entry by key
    let key_output = sandbox
        .new_assert_cmd("ledger")
        .arg("entry")
        .arg("fetch")
        .arg("contract")
        .arg(&contract_id)
        .arg("--network")
        .arg("testnet")
        .arg("--key")
        .arg(storage_key)
        .assert()
        .success()
        .stdout_as_str();
    let parsed_key_output: FullLedgerEntries =
        serde_json::from_str(&key_output).expect("Failed to parse JSON");
    assert!(!parsed_key_output.entries.is_empty());

    // get entry by key xdr
    let key_xdr_output = sandbox
        .new_assert_cmd("ledger")
        .arg("entry")
        .arg("fetch")
        .arg("contract")
        .arg(&contract_id)
        .arg("--network")
        .arg("testnet")
        .arg("--key-xdr")
        .arg(storage_key_xdr)
        .assert()
        .success()
        .stdout_as_str();

    let parsed_key_xdr_output: FullLedgerEntries =
        serde_json::from_str(&key_xdr_output).expect("Failed to parse JSON");
    assert!(!parsed_key_xdr_output.entries.is_empty());

    let expected_contract_data_key = expected_contract_ledger_key(&contract_id, storage_key).await;

    assert_eq!(parsed_key_output.entries[0].key, expected_contract_data_key);
    assert!(matches!(
        parsed_key_output.entries[0].val,
        LedgerEntryData::ContractData { .. }
    ));

    assert_eq!(
        parsed_key_xdr_output.entries[0].key,
        expected_contract_data_key
    );
    assert!(matches!(
        parsed_key_xdr_output.entries[0].val,
        LedgerEntryData::ContractData { .. }
    ));

    // the output should be the same regardless of key format
    assert_eq!(parsed_key_output.entries, parsed_key_xdr_output.entries);
}

// top level test
// todo: test --ttl, --claimable-id, --pool-id,
#[tokio::test]
async fn ledger_entry_wasm_hash() {
    let sandbox = &TestEnv::new();
    let test_account_alias = "test";
    let wasm = HELLO_WORLD;
    let wasm_path = wasm.path();
    let contract_wasm_hash = sandbox
        .new_assert_cmd("contract")
        .arg("upload")
        .arg("--wasm")
        .arg(wasm_path)
        .assert()
        .success()
        .stdout_as_str();

    deploy_contract(
        sandbox,
        HELLO_WORLD,
        DeployOptions {
            deployer: Some(test_account_alias.to_string()),
            ..Default::default()
        },
    )
    .await;

    // get the contract's wasm bytecode
    let output = sandbox
        .new_assert_cmd("ledger")
        .arg("entry")
        .arg("fetch")
        .arg("wasm")
        .arg(&contract_wasm_hash)
        .arg("--network")
        .arg("testnet")
        .assert()
        .success()
        .stdout_as_str();
    let parsed_output: FullLedgerEntries =
        serde_json::from_str(&output).expect("Failed to parse JSON");
    assert!(!parsed_output.entries.is_empty());

    let hash = Hash(
        padded_hex_from_str(&contract_wasm_hash, 32)
            .unwrap()
            .try_into()
            .unwrap(),
    );
    let expected_contract_key = LedgerKey::ContractCode(LedgerKeyContractCode { hash });

    assert_eq!(parsed_output.entries[0].key, expected_contract_key);
    assert!(matches!(
        parsed_output.entries[0].val,
        LedgerEntryData::ContractCode { .. }
    ));
    // key: ContractCode(LedgerKeyContractCode { hash: Hash(74a0a58bee2730d38dfaa547c0f3e64b1b76cf7d7e430373a9bf7aad122aff9f) }

    // assert_eq!(parsed_key_xdr_output.entries[0].key, expected_contract_data_key);
    // assert!(matches!(parsed_key_xdr_output.entries[0].val, LedgerEntryData::ContractData{ .. }));

    // // the output should be the same regardless of key format
    // assert_eq!(parsed_key_output.entries, parsed_key_xdr_output.entries);
}

#[tokio::test]
async fn ledger_entry_config_setting_id() {
    let sandbox = &TestEnv::new();
    let config_setting_ids = ConfigSettingId::VARIANTS;

    // for individual ids
    let output = sandbox
        .new_assert_cmd("ledger")
        .arg("entry")
        .arg("fetch")
        .arg("config")
        .arg((ConfigSettingId::ContractMaxSizeBytes as i32).to_string())
        .arg((ConfigSettingId::ContractDataEntrySizeBytes as i32).to_string())
        .arg("--network")
        .arg("testnet")
        .assert()
        .success()
        .stdout_as_str();
    let parsed_output: FullLedgerEntries =
        serde_json::from_str(&output).expect("Failed to parse JSON");
    assert!(!parsed_output.entries.is_empty());

    let expected_key_1 = LedgerKey::ConfigSetting(LedgerKeyConfigSetting {
        config_setting_id: ConfigSettingId::ContractMaxSizeBytes,
    });
    let expected_key_2 = LedgerKey::ConfigSetting(LedgerKeyConfigSetting {
        config_setting_id: ConfigSettingId::ContractDataEntrySizeBytes,
    });
    assert_eq!(parsed_output.entries[0].key, expected_key_1);
    assert_eq!(parsed_output.entries[1].key, expected_key_2);
    assert!(matches!(
        parsed_output.entries[0].val,
        LedgerEntryData::ConfigSetting { .. }
    ));
    assert!(matches!(
        parsed_output.entries[1].val,
        LedgerEntryData::ConfigSetting { .. }
    ));

    // for all ids
    let output = sandbox
        .new_assert_cmd("ledger")
        .arg("entry")
        .arg("fetch")
        .arg("config")
        .arg("--network")
        .arg("testnet")
        .assert()
        .success()
        .stdout_as_str();
    let parsed_output: FullLedgerEntries =
        serde_json::from_str(&output).expect("Failed to parse JSON");
    assert!(!parsed_output.entries.is_empty());
    assert_eq!(parsed_output.entries.len(), ConfigSettingId::variants().len());
}

#[ignore]
#[tokio::test]
async fn ledger_entry_ttl() {
    let sandbox = &TestEnv::new();
    let test_account_alias = "test";
    let contract_id = deploy_contract(
        sandbox,
        HELLO_WORLD,
        DeployOptions {
            deployer: Some(test_account_alias.to_string()),
            ..Default::default()
        },
    )
    .await;

    // get the contract's TTL
    let output = sandbox
        .new_assert_cmd("ledger")
        .arg("entry")
        .arg("fetch")
        .arg("--network")
        .arg("testnet")
        .arg("--ttl")
        .arg(contract_id)
        .assert()
        .success()
        .stdout_as_str();
    let parsed_output: FullLedgerEntries =
        serde_json::from_str(&output).expect("Failed to parse JSON");
    assert!(!parsed_output.entries.is_empty());
}

// Helper Fns
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

async fn issue_asset(sandbox: &TestEnv, test: &str, asset: &str, limit: u64, initial_balance: u64) {
    let client = sandbox.network.rpc_client().unwrap();
    let test_before = client.get_account(test).await.unwrap();
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "change-trust",
            "--line",
            asset,
            "--limit",
            limit.to_string().as_str(),
        ])
        .assert()
        .success()
        .stdout_as_str();

    sandbox
        .new_assert_cmd("tx")
        .args(["new", "set-options", "--set-required"])
        .assert()
        .success();
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "set-trustline-flags",
            "--asset",
            asset,
            "--trustor",
            test,
            "--set-authorize",
            "--source",
            "test1",
        ])
        .assert()
        .success();

    let after = client.get_account(test).await.unwrap();
    assert_eq!(test_before.num_sub_entries + 1, after.num_sub_entries);

    // Send a payment to the issuer
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "payment",
            "--destination",
            test,
            "--asset",
            asset,
            "--amount",
            initial_balance.to_string().as_str(),
            "--source=test1",
        ])
        .assert()
        .success();
}

async fn expected_account_ledger_key(account_addr: &str) -> (AccountId, LedgerKey) {
    let strkey = StrkeyPublicKeyEd25519::from_string(account_addr).unwrap().0;

    let uint256 = Uint256(strkey);
    let pk = PublicKey::PublicKeyTypeEd25519(uint256);
    let account_id = AccountId(pk);
    let ledger_key = LedgerKey::Account(LedgerKeyAccount {
        account_id: account_id.clone(),
    });
    (account_id, ledger_key)
}

async fn expected_contract_ledger_key(contract_id: &str, storage_key: &str) -> LedgerKey {
    let contract_bytes: [u8; 32] = Contract::from_string(contract_id).unwrap().0;
    let contract_id = Hash(contract_bytes);
    LedgerKey::ContractData(LedgerKeyContractData {
        contract: ScAddress::Contract(contract_id),
        key: ScVal::Symbol(storage_key.try_into().unwrap()),
        durability: ContractDataDurability::Persistent,
    })
}

async fn add_account_data(sandbox: &TestEnv, account_alias: &str, key: &str, value: &str) {
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "manage-data",
            "--data-name",
            key,
            "--data-value",
            value,
            "--source",
            account_alias,
        ])
        .assert()
        .success();
}
