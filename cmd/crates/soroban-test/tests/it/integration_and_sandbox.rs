use soroban_cli::commands::{config::identity, contract::fetch};
use soroban_test::TestEnv;

use crate::util::{
    add_test_seed, deploy_hello, is_rpc, network_passphrase_arg, rpc_url_arg, DEFAULT_PUB_KEY,
    DEFAULT_SECRET_KEY, DEFAULT_SEED_PHRASE, HELLO_WORLD,
};

#[test]
fn invoke_hello_world_with_lib_two() {
    TestEnv::with_default(|e| {
        let id = deploy_hello(e);
        let hello_world = HELLO_WORLD.to_string();
        let mut invoke_args = vec!["--id", &id, "--wasm", hello_world.as_str()];
        let args = vec!["--", "hello", "--world=world"];
        let res = if let (Some(rpc), Some(network_passphrase)) =
            (rpc_url_arg(), network_passphrase_arg())
        {
            invoke_args.push(&rpc);
            invoke_args.push(&network_passphrase);
            e.invoke(&[invoke_args, args].concat()).unwrap()
        } else {
            e.invoke(&[invoke_args, args].concat()).unwrap()
        };
        assert_eq!(res, r#"["Hello","world"]"#);
    });
}

#[test]
fn invoke_auth() {
    let sandbox = TestEnv::default();
    let id = &deploy_hello(&sandbox);
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

#[tokio::test]
async fn invoke_auth_with_identity() {
    let sandbox = TestEnv::default();
    sandbox
        .cmd::<identity::generate::Cmd>("test -d ")
        .run()
        .await
        .unwrap();
    let id = deploy_hello(&sandbox);
    sandbox
        .new_assert_cmd("contract")
        .arg("invoke")
        .arg("--id")
        .arg(id)
        .arg("--wasm")
        .arg(HELLO_WORLD.path())
        .arg("--")
        .arg("auth")
        .arg("--addr=test")
        .arg("--world=world")
        .assert()
        .stdout(format!("\"{DEFAULT_PUB_KEY}\"\n"))
        .success();
}

#[test]
fn contract_data_read_failure() {
    let sandbox = TestEnv::default();
    let id = deploy_hello(&sandbox);

    sandbox
        .new_assert_cmd("contract")
        .arg("read")
        .arg("--id")
        .arg(id)
        .arg("--key=COUNTER")
        .assert()
        .failure()
        .stderr(
            "error: no matching contract data entries were found for the specified contract id\n",
        );
}

#[test]
fn contract_data_read() {
    let sandbox = TestEnv::default();
    let id = &deploy_hello(&sandbox);

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
        .arg("--key=COUNTER")
        .assert()
        .success()
        .stdout("COUNTER,1\n");

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
        .arg("--key=COUNTER")
        .assert()
        .success()
        .stdout("COUNTER,2\n");
}

#[test]
fn invoke_hello_world_with_seed() {
    let sandbox = TestEnv::default();
    let identity = add_test_seed(sandbox.dir());
    invoke_with_source(&sandbox, &identity);
}

#[test]
fn invoke_with_seed() {
    let sandbox = TestEnv::default();
    invoke_with_source(&sandbox, DEFAULT_SEED_PHRASE);
}

#[test]
fn invoke_with_id() {
    let sandbox = TestEnv::default();
    let identity = add_test_seed(sandbox.dir());
    invoke_with_source(&sandbox, &identity);
}

#[test]
fn invoke_with_sk() {
    let sandbox = TestEnv::default();
    invoke_with_source(&sandbox, DEFAULT_SECRET_KEY);
}

fn invoke_with_source(sandbox: &TestEnv, source: &str) {
    let id = &deploy_hello(sandbox);
    let cmd = sandbox.invoke(&[
        "--source-account",
        source,
        "--id",
        id,
        "--wasm",
        HELLO_WORLD.path().to_str().unwrap(),
        &rpc_url_arg().unwrap_or_default(),
        &network_passphrase_arg().unwrap_or_default(),
        "--",
        "hello",
        "--world=world",
    ]);
    assert_eq!(cmd.unwrap(), "[\"Hello\",\"world\"]");

    // Invoke it again without providing the contract, to exercise the deployment
    let cmd = sandbox.invoke(&[
        "--source-account",
        source,
        "--id",
        id,
        "--",
        "hello",
        "--world=world",
    ]);
    assert_eq!(cmd.unwrap(), "[\"Hello\",\"world\"]");
}

#[tokio::test]
async fn fetch() {
    if !is_rpc() {
        return;
    }
    let e = TestEnv::default();
    let f = e.dir().join("contract.wasm");
    let id = deploy_hello(&e);
    let cmd = e.cmd_arr::<fetch::Cmd>(&["--id", &id, "--out-file", f.to_str().unwrap()]);
    cmd.run().await.unwrap();
    assert!(f.exists());
}
