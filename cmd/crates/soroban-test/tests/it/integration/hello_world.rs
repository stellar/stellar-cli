use soroban_cli::commands::{
    config::identity,
    contract::{self, fetch},
};
use soroban_test::TestEnv;

use crate::{integration::util::extend_contract, util::DEFAULT_SEED_PHRASE};

use super::util::{
    add_test_seed, deploy_hello, extend, network_passphrase, network_passphrase_arg, rpc_url,
    rpc_url_arg, DEFAULT_PUB_KEY, DEFAULT_PUB_KEY_1, DEFAULT_SECRET_KEY, HELLO_WORLD,
};

#[tokio::test]
#[ignore]
async fn invoke() {
    let sandbox = &TestEnv::default();
    let id = &deploy_hello(sandbox);
    extend_contract(sandbox, id, HELLO_WORLD).await;
    // Note that all functions tested here have no state
    invoke_hello_world(sandbox, id);
    invoke_hello_world_with_lib(sandbox, id).await;
    invoke_hello_world_with_lib_two(sandbox, id).await;
    invoke_auth(sandbox, id);
    invoke_auth_with_identity(sandbox, id).await;
    invoke_auth_with_different_test_account_fail(sandbox, id).await;
    // invoke_auth_with_different_test_account(sandbox, id);
    contract_data_read_failure(sandbox, id);
    invoke_with_seed(sandbox, id).await;
    invoke_with_sk(sandbox, id).await;
    // This does add an identity to local config
    invoke_with_id(sandbox, id).await;
    handles_kebab_case(sandbox, id).await;
    fetch(sandbox, id).await;
    invoke_prng_u64_in_range_test(sandbox, id).await;
}

fn invoke_hello_world(sandbox: &TestEnv, id: &str) {
    sandbox
        .new_assert_cmd("contract")
        .arg("invoke")
        .arg("--id")
        .arg(id)
        .arg("--")
        .arg("hello")
        .arg("--world=world")
        .assert()
        .stdout("[\"Hello\",\"world\"]\n")
        .success();
}

async fn invoke_hello_world_with_lib(e: &TestEnv, id: &str) {
    let mut cmd = contract::invoke::Cmd {
        contract_id: id.to_string(),
        slop: vec!["hello".into(), "--world=world".into()],
        ..Default::default()
    };

    cmd.config.network.rpc_url = rpc_url();
    cmd.config.network.network_passphrase = network_passphrase();

    let res = e.invoke_cmd(cmd).await.unwrap();
    assert_eq!(res, r#"["Hello","world"]"#);
}

async fn invoke_hello_world_with_lib_two(e: &TestEnv, id: &str) {
    let hello_world = HELLO_WORLD.to_string();
    let mut invoke_args = vec!["--id", id, "--wasm", hello_world.as_str()];
    let args = vec!["--", "hello", "--world=world"];
    let res =
        if let (Some(rpc), Some(network_passphrase)) = (rpc_url_arg(), network_passphrase_arg()) {
            invoke_args.push(&rpc);
            invoke_args.push(&network_passphrase);
            e.invoke(&[invoke_args, args].concat()).await.unwrap()
        } else {
            e.invoke(&[invoke_args, args].concat()).await.unwrap()
        };
    assert_eq!(res, r#"["Hello","world"]"#);
}

fn invoke_auth(sandbox: &TestEnv, id: &str) {
    sandbox
        .new_assert_cmd("contract")
        .arg("invoke")
        .arg("--id")
        .arg(id)
        .arg("--wasm")
        .arg(HELLO_WORLD.path())
        .arg("--")
        .arg("auth")
        .arg(&format!("--addr={DEFAULT_PUB_KEY}"))
        .arg("--world=world")
        .assert()
        .stdout(format!("\"{DEFAULT_PUB_KEY}\"\n"))
        .success();

    // Invoke it again without providing the contract, to exercise the deployment
    sandbox
        .new_assert_cmd("contract")
        .arg("invoke")
        .arg("--id")
        .arg(id)
        .arg("--")
        .arg("auth")
        .arg(&format!("--addr={DEFAULT_PUB_KEY}"))
        .arg("--world=world")
        .assert()
        .stdout(format!("\"{DEFAULT_PUB_KEY}\"\n"))
        .success();
}

async fn invoke_auth_with_identity(sandbox: &TestEnv, id: &str) {
    sandbox
        .cmd::<identity::generate::Cmd>("test -d ")
        .run()
        .await
        .unwrap();
    sandbox
        .new_assert_cmd("contract")
        .arg("invoke")
        .arg("--id")
        .arg(id)
        .arg("--wasm")
        .arg(HELLO_WORLD.path())
        .arg("--")
        .arg("auth")
        .arg("--addr")
        .arg(DEFAULT_PUB_KEY)
        .arg("--world=world")
        .assert()
        .stdout(format!("\"{DEFAULT_PUB_KEY}\"\n"))
        .success();
}

// fn invoke_auth_with_different_test_account(sandbox: &TestEnv, id: &str) {
//     sandbox
//         .new_assert_cmd("contract")
//         .arg("invoke")
//         .arg("--hd-path=1")
//         .arg("--id")
//         .arg(id)
//         .arg("--wasm")
//         .arg(HELLO_WORLD.path())
//         .arg("--")
//         .arg("auth")
//         .arg(&format!("--addr={DEFAULT_PUB_KEY_1}"))
//         .arg("--world=world")
//         .assert()
//         .stdout(format!("\"{DEFAULT_PUB_KEY_1}\"\n"))
//         .success();
// }

async fn invoke_auth_with_different_test_account_fail(sandbox: &TestEnv, id: &str) {
    let res = sandbox
        .invoke(&[
            "--hd-path=0",
            "--id",
            id,
            &rpc_url_arg().unwrap_or_default(),
            &network_passphrase_arg().unwrap_or_default(),
            "--",
            "auth",
            &format!("--addr={DEFAULT_PUB_KEY_1}"),
            "--world=world",
        ])
        .await;
    let e = res.unwrap_err();
    assert!(
        matches!(e, contract::invoke::Error::Rpc(_)),
        "Expected rpc error got {e:?}"
    );
}

fn contract_data_read_failure(sandbox: &TestEnv, id: &str) {
    sandbox
        .new_assert_cmd("contract")
        .arg("read")
        .arg("--id")
        .arg(id)
        .arg("--key=COUNTER")
        .arg("--durability=persistent")
        .assert()
        .failure()
        .stderr(
            "error: no matching contract data entries were found for the specified contract id\n",
        );
}

#[tokio::test]
async fn contract_data_read() {
    const KEY: &str = "COUNTER";
    let sandbox = &TestEnv::default();
    let id = &deploy_hello(sandbox);
    let res = sandbox.invoke(&["--id", id, "--", "inc"]).await.unwrap();
    assert_eq!(res.trim(), "1");
    extend(sandbox, id, Some(KEY)).await;

    sandbox
        .new_assert_cmd("contract")
        .arg("read")
        .arg("--id")
        .arg(id)
        .arg("--key")
        .arg(KEY)
        .arg("--durability=persistent")
        .assert()
        .success()
        .stdout(predicates::str::starts_with("COUNTER,1"));

    sandbox
        .new_assert_cmd("contract")
        .arg("invoke")
        .arg("--id")
        .arg(id)
        .arg("--")
        .arg("inc")
        .assert()
        .success();

    sandbox
        .new_assert_cmd("contract")
        .arg("read")
        .arg("--id")
        .arg(id)
        .arg("--key")
        .arg(KEY)
        .arg("--durability=persistent")
        .assert()
        .success()
        .stdout(predicates::str::starts_with("COUNTER,2"));
}

async fn invoke_with_seed(sandbox: &TestEnv, id: &str) {
    invoke_with_source(sandbox, DEFAULT_SEED_PHRASE, id).await;
}

async fn invoke_with_sk(sandbox: &TestEnv, id: &str) {
    invoke_with_source(sandbox, DEFAULT_SECRET_KEY, id).await;
}

async fn invoke_with_id(sandbox: &TestEnv, id: &str) {
    let identity = add_test_seed(sandbox.dir());
    invoke_with_source(sandbox, &identity, id).await;
}

async fn invoke_with_source(sandbox: &TestEnv, source: &str, id: &str) {
    let cmd = sandbox
        .invoke(&[
            "--source-account",
            source,
            "--id",
            id,
            "--",
            "hello",
            "--world=world",
        ])
        .await
        .unwrap();
    assert_eq!(cmd, "[\"Hello\",\"world\"]");
}

async fn handles_kebab_case(e: &TestEnv, id: &str) {
    assert!(e
        .invoke(&["--id", id, "--", "multi-word-cmd", "--contract-owner=world",])
        .await
        .is_ok());
}

async fn fetch(sandbox: &TestEnv, id: &str) {
    let f = sandbox.dir().join("contract.wasm");
    let cmd = sandbox.cmd_arr::<fetch::Cmd>(&["--id", id, "--out-file", f.to_str().unwrap()]);
    cmd.run().await.unwrap();
    assert!(f.exists());
}

async fn invoke_prng_u64_in_range_test(sandbox: &TestEnv, id: &str) {
    assert!(sandbox
        .invoke(&[
            "--id",
            id,
            "--wasm",
            HELLO_WORLD.path().to_str().unwrap(),
            "--",
            "prng_u64_in_range",
            "--low=0",
            "--high=100",
        ])
        .await
        .is_ok());
}
