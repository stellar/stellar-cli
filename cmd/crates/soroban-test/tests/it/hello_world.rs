use soroban_cli::commands::{
    config::identity,
    contract::{self, fetch},
};
use soroban_test::TestEnv;
use std::path::PathBuf;

use crate::util::{
    add_test_seed, is_rpc, network_passphrase, network_passphrase_arg, rpc_url, rpc_url_arg,
    DEFAULT_PUB_KEY, DEFAULT_PUB_KEY_1, DEFAULT_SECRET_KEY, DEFAULT_SEED_PHRASE, HELLO_WORLD,
    TEST_SALT,
};

#[test]
fn install_wasm_then_deploy_contract() {
    let sandbox = TestEnv::default();
    assert_eq!(deploy_hello(&sandbox), TEST_CONTRACT_ID);
}

const TEST_CONTRACT_ID: &str = "CBVTIVBYWAO2HNPNGKDCZW4OZYYESTKNGD7IPRTDGQSFJS4QBDQQJX3T";

fn deploy_hello(sandbox: &TestEnv) -> String {
    let hash = HELLO_WORLD.hash().unwrap();
    sandbox
        .new_assert_cmd("contract")
        .arg("install")
        .arg("--wasm")
        .arg(HELLO_WORLD.path())
        .assert()
        .success()
        .stdout(format!("{hash}\n"));

    let mut cmd: &mut assert_cmd::Command = &mut sandbox.new_assert_cmd("contract");

    cmd = cmd.arg("deploy").arg("--wasm-hash").arg(&format!("{hash}"));
    if is_rpc() {
        cmd = cmd.arg("--salt").arg(TEST_SALT);
    } else {
        cmd = cmd.arg("--id").arg(TEST_CONTRACT_ID);
    }
    cmd.assert()
        .success()
        .stdout(format!("{TEST_CONTRACT_ID}\n"));
    TEST_CONTRACT_ID.to_string()
}

#[test]
fn deploy_contract_with_wasm_file() {
    if is_rpc() {
        return;
    }
    TestEnv::default()
        .new_assert_cmd("contract")
        .arg("deploy")
        .arg("--wasm")
        .arg(HELLO_WORLD.path())
        .arg("--id=1")
        .assert()
        .success()
        .stdout("CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAD2KM\n");
}

#[test]
fn invoke_hello_world_with_deploy_first() {
    let sandbox = TestEnv::default();
    let id = deploy_hello(&sandbox);
    println!("{id}");
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

#[test]
fn invoke_hello_world() {
    let sandbox = TestEnv::default();
    let id = deploy_hello(&sandbox);
    sandbox
        .new_assert_cmd("contract")
        .arg("invoke")
        .arg("--id")
        .arg(id)
        .arg("--wasm")
        .arg(HELLO_WORLD.path())
        .arg("--")
        .arg("hello")
        .arg("--world=world")
        .assert()
        .stdout("[\"Hello\",\"world\"]\n")
        .success();
}

#[test]
fn invoke_hello_world_from_file() {
    let sandbox = TestEnv::default();
    let tmp_file = sandbox.temp_dir.join("world.txt");
    std::fs::write(&tmp_file, "world").unwrap();
    let id = deploy_hello(&sandbox);
    sandbox
        .new_assert_cmd("contract")
        .arg("invoke")
        .arg("--id")
        .arg(id)
        .arg("--wasm")
        .arg(HELLO_WORLD.path())
        .arg("--")
        .arg("hello")
        .arg("--world-file-path")
        .arg(&tmp_file)
        .assert()
        .stdout("[\"Hello\",\"world\"]\n")
        .success();
}

#[test]
fn invoke_hello_world_from_file_fail() {
    let sandbox = TestEnv::default();
    let tmp_file = sandbox.temp_dir.join("world.txt");
    std::fs::write(&tmp_file, "world").unwrap();
    let id = deploy_hello(&sandbox);
    sandbox
        .new_assert_cmd("contract")
        .arg("invoke")
        .arg("--id")
        .arg(id)
        .arg("--wasm")
        .arg(HELLO_WORLD.path())
        .arg("--")
        .arg("hello")
        .arg("--world-file-path")
        .arg(&tmp_file)
        .arg("--world=hello")
        .assert()
        .stderr(predicates::str::contains("error: the argument '--world-file-path <world-file-path>' cannot be used with '--world <Symbol>'"))
        .failure();
}

#[test]
fn invoke_hello_world_with_lib() {
    TestEnv::with_default(|e| {
        let id = deploy_hello(e);
        let mut cmd = contract::invoke::Cmd {
            contract_id: id,
            slop: vec!["hello".into(), "--world=world".into()],
            ..Default::default()
        };

        cmd.config.network.rpc_url = rpc_url();
        cmd.config.network.network_passphrase = network_passphrase();

        let res = e.invoke_cmd(cmd).unwrap();
        assert_eq!(res, r#"["Hello","world"]"#);
    });
}

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
// #[test]
// fn invoke_hello_world_with_lib_three() {
//     let sandbox = TestEnv::default();
//     let builder  = invoke::CmdBuilder::new().contract_id("1").wasm(HELLO_WORLD.path()).function("hello").slop(["--hello=world"]).build();
//     std::env::set_current_dir(sandbox.dir()).unwrap();
//     assert_eq!(res.run_in_sandbox().unwrap(), r#"["Hello","world"]"#);
// }

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
fn invoke_auth_with_different_test_account() {
    let sandbox = TestEnv::default();
    let id = deploy_hello(&sandbox);
    sandbox
        .new_assert_cmd("contract")
        .arg("invoke")
        .arg("--hd-path=1")
        .arg("--id")
        .arg(id)
        .arg("--wasm")
        .arg(HELLO_WORLD.path())
        .arg("--")
        .arg("auth")
        .arg(&format!("--addr={DEFAULT_PUB_KEY_1}"))
        .arg("--world=world")
        .assert()
        .stdout(format!("\"{DEFAULT_PUB_KEY_1}\"\n"))
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
        .arg("--durability=persistent")
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
        .arg("--durability=persistent")
        .assert()
        .success()
        .stdout("COUNTER,1,4096\n");

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
        .arg("--durability=persistent")
        .assert()
        .success()
        .stdout("COUNTER,2,4096\n");
}

#[test]
fn invoke_auth_with_different_test_account_fail() {
    let sandbox = TestEnv::default();
    let id = &deploy_hello(&sandbox);
    let res = sandbox.invoke(&[
        "--hd-path=1",
        "--id",
        id,
        "--wasm",
        HELLO_WORLD.path().to_str().unwrap(),
        &rpc_url_arg().unwrap_or_default(),
        &network_passphrase_arg().unwrap_or_default(),
        "--",
        "auth",
        &format!("--addr={DEFAULT_PUB_KEY}"),
        "--world=world",
    ]);
    assert!(res.is_err());
    if let Err(e) = res {
        assert!(
            matches!(e, contract::invoke::Error::Host(_)),
            "Expected host error got {e:?}"
        );
    };
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

#[test]
fn handles_kebab_case() {
    let e = TestEnv::default();
    let id = deploy_hello(&e);
    assert!(e
        .invoke(&[
            "--id",
            &id,
            "--wasm",
            HELLO_WORLD.path().to_str().unwrap(),
            "--",
            "multi-word-cmd",
            "--contract-owner=world",
        ])
        .is_ok());
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

#[test]
fn build() {
    let sandbox = TestEnv::default();

    let cargo_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let hello_world_contract_path =
        cargo_dir.join("tests/fixtures/test-wasms/hello_world/Cargo.toml");
    sandbox
        .new_assert_cmd("contract")
        .arg("build")
        .arg("--manifest-path")
        .arg(hello_world_contract_path)
        .arg("--profile")
        .arg("test-wasms")
        .arg("--package")
        .arg("test_hello_world")
        .assert()
        .success();
}

#[test]
fn invoke_prng_u64_in_range_test() {
    let sandbox = TestEnv::default();
    let res = sandbox
        .new_assert_cmd("contract")
        .arg("deploy")
        .arg("--wasm")
        .arg(HELLO_WORLD.path())
        .assert()
        .success();
    let stdout = String::from_utf8(res.get_output().stdout.clone()).unwrap();
    let id = stdout.trim_end();
    println!("{id}");
    sandbox
        .new_assert_cmd("contract")
        .arg("invoke")
        .arg("--id")
        .arg(id)
        .arg("--")
        .arg("prng_u64_in_range")
        .arg("--low=0")
        .arg("--high=100")
        .assert()
        .success();
}
