use sha2::{Digest, Sha256};
use soroban_cli::{
    config::{address::UnresolvedMuxedAccount, locator},
    tx::builder::TxExt,
    xdr::{
        self, AccountId, AlphaNum4, Asset, AssetCode4, ChangeTrustAsset, ChangeTrustOp,
        ClaimPredicate, ClaimableBalanceId, Claimant, ClaimantV0, ContractDataDurability,
        CreateClaimableBalanceOp, CreateClaimableBalanceResult, Hash, LedgerEntryData, LedgerKey,
        LedgerKeyAccount, LedgerKeyClaimableBalance, LedgerKeyContractCode, LedgerKeyContractData,
        LedgerKeyData, LedgerKeyLiquidityPool, LedgerKeyTrustLine, Limits,
        LiquidityPoolConstantProductParameters, LiquidityPoolParameters, Operation, OperationBody,
        OperationResult, OperationResultTr, PoolId, PublicKey, ScAddress, ScVal, String64, StringM,
        TransactionEnvelope, TransactionResult, TransactionResultResult, TrustLineAsset, Uint256,
        VecM, WriteXdr,
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
async fn ledger_entry_account_with_alias() {
    let sandbox = &TestEnv::new();
    let account_alias = "new_account";
    let new_account_addr = new_account(sandbox, account_alias);
    let output = sandbox
        .new_assert_cmd("ledger")
        .arg("entry")
        .arg("fetch")
        .arg("account")
        .arg("--account")
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
async fn ledger_entry_account_with_account_addr() {
    let sandbox = &TestEnv::new();
    let new_account_addr = new_account(sandbox, "new_account");
    let output = sandbox
        .new_assert_cmd("ledger")
        .arg("entry")
        .arg("fetch")
        .arg("account")
        .arg("--account")
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
async fn ledger_entry_trustline_asset_usdc() {
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
        &issuer_alias,
        asset,
        limit,
        initial_balance,
    )
    .await;

    let output = sandbox
        .new_assert_cmd("ledger")
        .arg("entry")
        .arg("fetch")
        .arg("trustline")
        .arg("--account")
        .arg(test_account_alias)
        .arg("--network")
        .arg("testnet")
        .arg("--asset")
        .arg(asset)
        .assert()
        .success()
        .stdout_as_str();

    let (account_id, _expected_account_key) =
        expected_account_ledger_key(&test_account_address).await;
    let issuer_account_id = get_account_id(&issuer_address);

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

    let trustline_entry = &parsed.entries[0];
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
        .arg("data")
        .arg("--account")
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
        .arg("contract-data")
        .arg("--contract")
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
        .arg("contract-data")
        .arg("--contract")
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
#[tokio::test]
async fn ledger_entry_contract_code() {
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
        .arg("contract-code")
        .arg("--wasm-hash")
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
}

#[tokio::test]
async fn ledger_entry_claimable_balance() {
    let sandbox = &TestEnv::new();
    // create a claimable balance
    let sender_alias = "test";
    let sender = test_address(sandbox);
    let claimant = new_account(sandbox, "claimant");
    let tx_env = claimable_balance_tx_env(&sender, &claimant);
    let tx_xdr = tx_env.to_xdr_base64(Limits::none()).unwrap();
    let updated_tx = update_seq_number(sandbox, &tx_xdr);
    let tx_output = sign_and_send(sandbox, sender_alias, &updated_tx).await;
    let response: GetTransactionResponse =
        serde_json::from_str(&tx_output).expect("Failed to parse JSON");
    let id = extract_claimable_balance_id(response).unwrap();

    // fetch the claimable-balance
    let output = sandbox
        .new_assert_cmd("ledger")
        .arg("entry")
        .arg("fetch")
        .arg("claimable-balance")
        .arg("--id")
        .arg(id.to_string())
        .arg("--network")
        .arg("local")
        .assert()
        .success()
        .stdout_as_str();
    let parsed_output: FullLedgerEntries =
        serde_json::from_str(&output).expect("Failed to parse JSON");
    assert!(!parsed_output.entries.is_empty());
    let expected_key = LedgerKey::ClaimableBalance(LedgerKeyClaimableBalance {
        balance_id: ClaimableBalanceId::ClaimableBalanceIdTypeV0(id),
    });
    assert_eq!(parsed_output.entries[0].key, expected_key);
    assert!(matches!(
        parsed_output.entries[0].val,
        LedgerEntryData::ClaimableBalance { .. }
    ));
}

#[tokio::test]
async fn ledger_entry_liquidity_pool() {
    let sandbox = &TestEnv::new();
    let test_account_alias = "test";
    let test_account_address = test_address(sandbox);
    // issue usdc
    let issuer_alias = "test1";
    let issuer_address = new_account(sandbox, issuer_alias);
    let asset = &format!("usdc:{issuer_address}");
    let limit = 100_000;
    let initial_balance = 100;
    issue_asset(
        sandbox,
        &test_account_address,
        &issuer_alias,
        asset,
        limit,
        initial_balance,
    )
    .await;

    // create liquidity pool
    let (tx_env, pool_id) = liquidity_pool_tx_env(&test_account_address, &issuer_address);
    let tx_xdr = tx_env.to_xdr_base64(Limits::none()).unwrap();
    let updated_tx = update_seq_number(sandbox, &tx_xdr);
    sign_and_send(sandbox, test_account_alias, &updated_tx).await;

    // fetch the liquidity pool
    let output = sandbox
        .new_assert_cmd("ledger")
        .arg("entry")
        .arg("fetch")
        .arg("liquidity-pool")
        .arg("--id")
        .arg(pool_id.to_string())
        .arg("--network")
        .arg("local")
        .assert()
        .success()
        .stdout_as_str();
    let parsed_output: FullLedgerEntries =
        serde_json::from_str(&output).expect("Failed to parse JSON");
    assert!(!parsed_output.entries.is_empty());
    let expected_key = LedgerKey::LiquidityPool(LedgerKeyLiquidityPool {
        liquidity_pool_id: PoolId(Hash(pool_id.0)),
    });
    assert_eq!(parsed_output.entries[0].key, expected_key);
    assert!(matches!(
        parsed_output.entries[0].val,
        LedgerEntryData::LiquidityPool { .. }
    ));
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
        contract: ScAddress::Contract(contract_id.into()),
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

fn claimable_balance_tx_env(sender: &str, destination: &str) -> TransactionEnvelope {
    let destination_id = get_account_id(&destination);
    let claimant = Claimant::ClaimantTypeV0(ClaimantV0 {
        destination: destination_id,
        predicate: ClaimPredicate::Unconditional,
    });
    let claimants = VecM::try_from(vec![claimant]).unwrap();
    let create_op = Operation {
        source_account: None,
        body: OperationBody::CreateClaimableBalance(CreateClaimableBalanceOp {
            asset: Asset::Native,
            amount: 10_000_000,
            claimants: claimants,
        }),
    };

    let source: UnresolvedMuxedAccount = sender.parse().unwrap();
    let resolved_source = source
        .resolve_muxed_account_sync(&locator::Args::default(), None)
        .unwrap();

    xdr::Transaction::new_tx(resolved_source, 1000, 1, create_op).into()
}

fn liquidity_pool_tx_env(
    test_account_address: &str,
    usdc_issuer_address: &str,
) -> (TransactionEnvelope, Uint256) {
    let issuer_account_id = get_account_id(&usdc_issuer_address);
    let usdc_asset = Asset::CreditAlphanum4(AlphaNum4 {
        asset_code: AssetCode4(*b"usdc"),
        issuer: issuer_account_id,
    });

    let asset_a = Asset::Native;
    let asset_b = usdc_asset;
    let fee = 30;

    let line = ChangeTrustAsset::PoolShare(LiquidityPoolParameters::LiquidityPoolConstantProduct(
        LiquidityPoolConstantProductParameters {
            asset_a: asset_a.clone(),
            asset_b: asset_b.clone(),
            fee,
        },
    ));
    let op = Operation {
        source_account: None,
        body: OperationBody::ChangeTrust(ChangeTrustOp {
            line: line,
            limit: i64::MAX,
        }),
    };

    let source: UnresolvedMuxedAccount = test_account_address.parse().unwrap();
    let resolved_source = source
        .resolve_muxed_account_sync(&locator::Args::default(), None)
        .unwrap();

    let tx = xdr::Transaction::new_tx(resolved_source, 1000, 1, op).into();

    let pool_id = compute_pool_id(asset_a.clone(), asset_b.clone(), fee);

    (tx, pool_id)
}

fn update_seq_number(sandbox: &TestEnv, tx_xdr: &str) -> String {
    sandbox
        .new_assert_cmd("tx")
        .arg("update")
        .arg("seq-num")
        .arg("next")
        .write_stdin(tx_xdr.as_bytes())
        .assert()
        .success()
        .stdout_as_str()
}

async fn sign_and_send(sandbox: &TestEnv, sign_with: &str, tx: &str) -> String {
    let tx_signed = sandbox
        .new_assert_cmd("tx")
        .arg("sign")
        .arg("--sign-with-key")
        .arg(sign_with)
        .write_stdin(tx.as_bytes())
        .assert()
        .success()
        .stdout_as_str();

    sandbox
        .new_assert_cmd("tx")
        .arg("send")
        .write_stdin(tx_signed.as_bytes())
        .assert()
        .success()
        .stdout(predicates::str::contains("SUCCESS"))
        .stdout_as_str()
}

fn extract_claimable_balance_id(response: GetTransactionResponse) -> Option<Hash> {
    if let Some(result) = response.result {
        if let TransactionResult {
            result: TransactionResultResult::TxSuccess(results),
            ..
        } = result
        {
            if let Some(OperationResult::OpInner(OperationResultTr::CreateClaimableBalance(
                CreateClaimableBalanceResult::Success(
                    ClaimableBalanceId::ClaimableBalanceIdTypeV0(hash),
                ),
            ))) = results.first()
            {
                return Some(hash.clone());
            }
        }
    }
    None
}

fn compute_pool_id(asset_a: Asset, asset_b: Asset, fee: i32) -> Uint256 {
    let (asset_a, asset_b) = if asset_a < asset_b {
        (asset_a, asset_b)
    } else {
        (asset_b, asset_a)
    };

    let pool_params = LiquidityPoolParameters::LiquidityPoolConstantProduct(
        LiquidityPoolConstantProductParameters {
            asset_a,
            asset_b,
            fee,
        },
    );

    let mut hasher = Sha256::new();
    hasher.update(pool_params.to_xdr(Limits::none()).unwrap());
    let hash = hasher.finalize();

    Uint256(hash.into())
}
