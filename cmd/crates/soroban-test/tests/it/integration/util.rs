use soroban_cli::commands;
use soroban_test::{TestEnv, Wasm};
use std::fmt::Display;

pub const HELLO_WORLD: &Wasm = &Wasm::Custom("test-wasms", "test_hello_world");
pub const CUSTOM_TYPES: &Wasm = &Wasm::Custom("test-wasms", "test_custom_types");
pub const CUSTOM_ACCOUNT: &Wasm = &Wasm::Custom("test-wasms", "test_custom_account");
pub const SWAP: &Wasm = &Wasm::Custom("test-wasms", "test_swap");

pub async fn invoke_with_roundtrip<D>(e: &TestEnv, id: &str, func: &str, data: D)
where
    D: Display,
{
    let data = data.to_string();
    println!("{data}");
    let res = e
        .invoke_with_test(&["--id", id, "--", func, &format!("--{func}"), &data])
        .await
        .unwrap();
    assert_eq!(res, data);
}

pub const TEST_SALT: &str = "f55ff16f66f43360266b95db6f8fec01d76031054306ae4a4b380598f6cfd114";

pub async fn deploy_hello(sandbox: &TestEnv) -> String {
    deploy_contract(sandbox, HELLO_WORLD).await
}

pub async fn deploy_custom(sandbox: &TestEnv) -> String {
    deploy_contract(sandbox, CUSTOM_TYPES).await
}

pub async fn deploy_swap(sandbox: &TestEnv) -> String {
    deploy_contract(sandbox, SWAP).await
}

pub async fn deploy_custom_account(sandbox: &TestEnv) -> String {
    deploy_contract(sandbox, CUSTOM_ACCOUNT).await
}

pub async fn deploy_contract(sandbox: &TestEnv, wasm: &Wasm<'static>) -> String {
    let cmd = sandbox.cmd_with_config::<_, commands::contract::deploy::wasm::Cmd>(&[
        "--fee",
        "1000000",
        "--wasm",
        &wasm.path().to_string_lossy(),
        "--salt",
        TEST_SALT,
        "--ignore-checks",
    ]);
    sandbox
        .run_cmd_with(cmd, "test")
        .await
        .unwrap()
        .into_result()
        .unwrap()
}

pub async fn extend_contract(sandbox: &TestEnv, id: &str) {
    extend(sandbox, id, None).await;
}

pub async fn extend(sandbox: &TestEnv, id: &str, value: Option<&str>) {
    let mut args = vec![
        "--id",
        id,
        "--durability",
        "persistent",
        "--ledgers-to-extend",
        "100000",
    ];
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
