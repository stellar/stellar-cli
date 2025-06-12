use sha2::{Digest, Sha256};
use soroban_cli::{
    config::{address::UnresolvedMuxedAccount, locator},
    tx::builder::TxExt,
    xdr::{
        self, AccountId, AlphaNum4, Asset, AssetCode4, ChangeTrustAsset, ChangeTrustOp,
        ClaimPredicate, ClaimableBalanceId, Claimant, ClaimantV0, ConfigSettingId,
        ContractDataDurability, CreateClaimableBalanceOp, CreateClaimableBalanceResult, Hash,
        LedgerEntryData, LedgerKey, LedgerKeyAccount, LedgerKeyClaimableBalance,
        LedgerKeyConfigSetting, LedgerKeyContractCode, LedgerKeyContractData, LedgerKeyData,
        LedgerKeyLiquidityPool, LedgerKeyTrustLine, Limits, LiquidityPoolConstantProductParameters,
        LiquidityPoolParameters, Operation, OperationBody, OperationResult, OperationResultTr,
        PoolId, PublicKey, ScAddress, ScVal, String64, StringM, TransactionEnvelope,
        TransactionResult, TransactionResultResult, TrustLineAsset, Uint256, VecM, WriteXdr,
    },
};

use soroban_rpc::FullLedgerEntries;
use soroban_rpc::GetTransactionResponse;
use soroban_spec_tools::utils::padded_hex_from_str;
use soroban_test::AssertExt;
use soroban_test::TestEnv;
use stellar_strkey::{ed25519::PublicKey as StrkeyPublicKeyEd25519, Contract};

use crate::integration::util::{deploy_contract, test_address, DeployOptions, HELLO_WORLD};

// account data tests
// todo: test with --offer,
#[tokio::test]
async fn ledger_latest() {
    let sandbox = &TestEnv::new();
    let account_alias = "new_account";
    let new_account_addr = new_account(sandbox, account_alias);
    let output = sandbox
        .new_assert_cmd("ledger")
        .arg("latest")
        .arg("--network")
        .arg("testnet")
        .assert()
        .success()
        .stdout(predicates::str::contains("Sequence:"))
        .stdout(predicates::str::contains("Protocol Version:"))
        .stdout(predicates::str::contains("ID:"));

    // let (_, expected_key) = expected_account_ledger_key(&new_account_addr).await;
    // let parsed: FullLedgerEntries = serde_json::from_str(&output).expect("Failed to parse JSON");

    // assert!(!parsed.entries.is_empty());
    // assert_eq!(parsed.entries[0].key, expected_key);
    // assert!(matches!(
    //     parsed.entries[0].val,
    //     LedgerEntryData::Account { .. }
    // ));
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

async fn issue_asset(
    sandbox: &TestEnv,
    test_addr: &str,
    issuer_alias: &str,
    asset: &str,
    limit: u64,
    initial_balance: u64,
) {
    let client = sandbox.network.rpc_client().unwrap();
    let test_before = client.get_account(test_addr).await.unwrap();
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

    let after = client.get_account(test_addr).await.unwrap();
    assert_eq!(test_before.num_sub_entries + 1, after.num_sub_entries);

    // Send asset to the test
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "payment",
            "--destination",
            test_addr,
            "--asset",
            asset,
            "--amount",
            initial_balance.to_string().as_str(),
            "--source",
            issuer_alias,
        ])
        .assert()
        .success();
}

async fn expected_account_ledger_key(account_addr: &str) -> (AccountId, LedgerKey) {
    let account_id = get_account_id(account_addr);
    let ledger_key = LedgerKey::Account(LedgerKeyAccount {
        account_id: account_id.clone(),
    });
    (account_id, ledger_key)
}

fn get_account_id(account_addr: &str) -> AccountId {
    let strkey = StrkeyPublicKeyEd25519::from_string(account_addr).unwrap().0;

    let uint256 = Uint256(strkey);
    let pk = PublicKey::PublicKeyTypeEd25519(uint256);
    AccountId(pk)
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
