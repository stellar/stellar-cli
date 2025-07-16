use soroban_cli::{
    commands,
    xdr::{Limits, WriteXdr},
};
use soroban_test::{AssertExt, TestEnv, Wasm, LOCAL_NETWORK_PASSPHRASE, TEST_ACCOUNT};
use std::{env, fmt::Display};

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
    #[cfg(feature = "version_lt_23")]
    SimOnly,
}

impl Display for DeployKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeployKind::BuildOnly => write!(f, "--build-only"),
            DeployKind::Normal => write!(f, ""),
            #[cfg(feature = "version_lt_23")]
            DeployKind::SimOnly => write!(f, "--sim-only"),
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

pub fn setup_env_for_sandbox(sandbox: &TestEnv) {
    env::set_var("SOROBAN_ACCOUNT", TEST_ACCOUNT);
    env::set_var("SOROBAN_RPC_URL", sandbox.network.rpc_url.clone());
    env::set_var("SOROBAN_NETWORK_PASSPHRASE", LOCAL_NETWORK_PASSPHRASE);

    env::set_var(
        "XDG_CONFIG_HOME",
        sandbox.temp_dir.join("config").as_os_str(),
    );
    env::set_var("XDG_DATA_HOME", sandbox.temp_dir.join("data").as_os_str());
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
    setup_env_for_sandbox(sandbox);
    let mut cmd = sandbox.cmd_with_config::<_, commands::contract::deploy::wasm::Cmd>(
        &[
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

    let res = sandbox
        .run_cmd_with(cmd, deployer.as_deref().unwrap_or("test"))
        .await
        .unwrap();
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
