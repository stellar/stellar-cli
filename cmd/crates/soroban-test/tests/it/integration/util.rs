use soroban_cli::{
    commands,
    xdr::{Limits, WriteXdr},
};
use soroban_test::{AssertExt, TestEnv, Wasm};
use std::fmt::Display;

pub const HELLO_WORLD: &Wasm = &Wasm::Custom("test-wasms", "test_hello_world");
pub const CONSTRUCTOR: &Wasm = &Wasm::Custom("test-wasms", "test_constructor");
pub const CUSTOM_TYPES: &Wasm = &Wasm::Custom("test-wasms", "test_custom_types");
pub const CUSTOM_ACCOUNT: &Wasm = &Wasm::Custom("test-wasms", "test_custom_account");
pub const SWAP: &Wasm = &Wasm::Custom("test-wasms", "test_swap");

pub async fn invoke(sandbox: &TestEnv, id: &str, func: &str, data: &str) -> String {
    sandbox
        .invoke_with_test(&["--id", id, "--", func, &format!("--{func}"), data])
        .await
        .unwrap()
}
pub async fn invoke_with_roundtrip<D>(e: &TestEnv, id: &str, func: &str, data: D)
where
    D: Display,
{
    let data = data.to_string();
    println!("{data}");
    let res = invoke(e, id, func, &data).await;
    assert_eq!(res, data);
}

#[derive(Default)]
pub enum DeployKind {
    BuildOnly,
    #[default]
    Normal,
}

impl Display for DeployKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeployKind::BuildOnly => write!(f, "--build-only"),
            DeployKind::Normal => write!(f, ""),
        }
    }
}

pub async fn deploy_hello(sandbox: &TestEnv) -> String {
    deploy_contract(sandbox, HELLO_WORLD, DeployOptions::default()).await
}

pub async fn deploy_custom(sandbox: &TestEnv) -> String {
    deploy_contract(sandbox, CUSTOM_TYPES, DeployOptions::default()).await
}

pub async fn deploy_swap(sandbox: &TestEnv) -> String {
    deploy_contract(sandbox, SWAP, DeployOptions::default()).await
}

pub async fn deploy_custom_account(sandbox: &TestEnv) -> String {
    deploy_contract(sandbox, CUSTOM_ACCOUNT, DeployOptions::default()).await
}

#[derive(Default)]
pub struct DeployOptions {
    pub kind: DeployKind,
    pub deployer: Option<String>,
    pub salt: Option<String>,
}

pub async fn deploy_contract(
    sandbox: &TestEnv,
    wasm: &Wasm<'static>,
    DeployOptions {
        kind,
        deployer,
        salt,
    }: DeployOptions,
) -> String {
    let mut cmd = sandbox.cmd_with_config::<_, commands::contract::deploy::wasm::Cmd>(
        &[
            "--config-dir",
            &sandbox.config_dir().to_string_lossy(),
            "--fee",
            "1000000",
            "--wasm",
            &wasm.path().to_string_lossy(),
            "--ignore-checks",
            &kind.to_string(),
        ],
        None,
    );
    cmd.salt = salt;

    let config = sandbox.clone_config(deployer.as_deref().unwrap_or("test"));
    let res = cmd.execute(&config, false, false).await.unwrap();

    match kind {
        DeployKind::Normal => (),
        _ => match res.to_envelope() {
            commands::txn_result::TxnEnvelopeResult::TxnEnvelope(e) => {
                return e.to_xdr_base64(Limits::none()).unwrap()
            }
            commands::txn_result::TxnEnvelopeResult::Res(_) => todo!(),
        },
    }
    res.into_result().unwrap().to_string()
}

pub async fn extend_contract(sandbox: &TestEnv, id: &str) {
    extend(sandbox, id, None).await;
}

pub async fn extend(sandbox: &TestEnv, id: &str, value: Option<&str>) {
    let mut args = vec!["--id", id, "--ledgers-to-extend", "100001"];

    if let Some(value) = value {
        args.push("--key");
        args.push(value);
    }

    sandbox
        .new_assert_cmd("contract")
        .arg("extend")
        .args(args)
        .assert()
        .success();
}

pub fn test_address(sandbox: &TestEnv) -> String {
    sandbox
        .new_assert_cmd("keys")
        .arg("address")
        .arg("test")
        .assert()
        .success()
        .stdout_as_str()
}

pub fn get_sponsoring_count(account: &soroban_cli::xdr::AccountEntry) -> u32 {
    match &account.ext {
        soroban_cli::xdr::AccountEntryExt::V1(v1) => match &v1.ext {
            soroban_cli::xdr::AccountEntryExtensionV1Ext::V2(v2) => v2.num_sponsoring,
            _ => panic!("Account extension V1 should have V2 extension for sponsoring"),
        },
        _ => panic!("Account should have V1 extension for sponsoring"),
    }
}

pub fn new_account(sandbox: &TestEnv, name: &str) -> String {
    sandbox.generate_account(name, None).assert().success();
    sandbox
        .new_assert_cmd("keys")
        .args(["address", name])
        .assert()
        .success()
        .stdout_as_str()
}

pub fn gen_account_no_fund(sandbox: &TestEnv, name: &str) -> String {
    sandbox
        .new_assert_cmd("keys")
        .args(["generate", name])
        .assert()
        .success();
    sandbox
        .new_assert_cmd("keys")
        .args(["address", name])
        .assert()
        .success()
        .stdout_as_str()
}

// returns test and test1 addresses
pub fn setup_accounts(sandbox: &TestEnv) -> (String, String) {
    (test_address(sandbox), new_account(sandbox, "test1"))
}

pub async fn issue_asset(
    sandbox: &TestEnv,
    test: &str,
    asset: &str,
    limit: u64,
    initial_balance: u64,
) {
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
        .success();

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
    println!("aa");
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
