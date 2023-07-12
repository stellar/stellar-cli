use soroban_cli::commands::{
    config::identity,
    contract::{self, fetch},
};
use soroban_test::TestEnv;

use crate::util::{
    add_test_seed, DEFAULT_PUB_KEY, DEFAULT_PUB_KEY_1, DEFAULT_SECRET_KEY, DEFAULT_SEED_PHRASE,
    HELLO_WORLD,
};

#[test]
fn install_wasm_then_deploy_contract() {
    let hash = HELLO_WORLD.hash().unwrap();
    let sandbox = TestEnv::default();
    sandbox
        .new_assert_cmd("contract")
        .arg("install")
        .arg("--wasm")
        .arg(HELLO_WORLD.path())
        .assert()
        .success()
        .stdout(format!("{hash}\n"));

    sandbox
        .new_assert_cmd("contract")
        .arg("deploy")
        .arg("--wasm-hash")
        .arg(&format!("{hash}"))
        .arg("--id=1")
        .assert()
        .success()
        .stdout("CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAD2KM\n");
}

#[test]
fn deploy_contract_with_wasm_file() {
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
        .arg("hello")
        .arg("--world=world")
        .assert()
        .stdout("[\"Hello\",\"world\"]\n")
        .success();
}

#[test]
fn invoke_hello_world() {
    let sandbox = TestEnv::default();
    sandbox
        .new_assert_cmd("contract")
        .arg("invoke")
        .arg("--id=1")
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
fn invoke_hello_world_with_lib() {
    TestEnv::with_default(|e| {
        let cmd = contract::invoke::Cmd {
            contract_id: "1".to_string(),
            wasm: Some(HELLO_WORLD.path()),
            slop: vec!["hello".into(), "--world=world".into()],
            ..Default::default()
        };
        let res = e.invoke_cmd(cmd).unwrap();
        assert_eq!(res, r#"["Hello","world"]"#);
    });
}

#[test]
fn invoke_hello_world_with_lib_two() {
    TestEnv::with_default(|e| {
        let res = e
            .invoke(&[
                "--id=1",
                "--wasm",
                &HELLO_WORLD.to_string(),
                "--",
                "hello",
                "--world=world",
            ])
            .unwrap();
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
    sandbox
        .new_assert_cmd("contract")
        .arg("invoke")
        .arg("--id=1")
        .arg("--wasm")
        .arg(HELLO_WORLD.path())
        .arg("--")
        .arg("auth")
        .arg(&format!("--addr={DEFAULT_PUB_KEY}"))
        .arg("--world=world")
        .assert()
        .stdout(format!("\"{DEFAULT_PUB_KEY}\"\n"))
        .success();
}

#[test]
fn invoke_auth_with_identity() {
    let sandbox = TestEnv::default();

    sandbox
        .cmd::<identity::generate::Cmd>("test -d ")
        .run()
        .unwrap();
    sandbox
        .new_assert_cmd("contract")
        .arg("invoke")
        .arg("--id=1")
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
    sandbox
        .new_assert_cmd("contract")
        .arg("invoke")
        .arg("--hd-path=1")
        .arg("--id=1")
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
fn invoke_auth_with_different_test_account_fail() {
    let sandbox = TestEnv::default();

    let res = sandbox.invoke(&[
        "--hd-path=1",
        "--id=1",
        "--wasm",
        HELLO_WORLD.path().to_str().unwrap(),
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
    let cmd = sandbox.invoke(&[
        "--source-account",
        source,
        "--id=1",
        "--wasm",
        HELLO_WORLD.path().to_str().unwrap(),
        "--",
        "hello",
        "--world=world",
    ]);
    assert_eq!(cmd.unwrap(), "[\"Hello\",\"world\"]");
}

#[test]
fn handles_kebab_case() {
    assert!(TestEnv::default()
        .invoke(&[
            "--id=1",
            "--wasm",
            HELLO_WORLD.path().to_str().unwrap(),
            "--",
            "multi-word-cmd",
            "--contract-owner=world",
        ])
        .is_ok());
}

#[ignore]
#[tokio::test]
async fn fetch() {
    // TODO: Currently this test fetches a live contract from futurenet. This obviously depends on
    // futurenet for the test to work, which is not great. But also means that if we are upgrading
    // the XDR ahead of a futurenet upgrade, this test will pass. Oof. :(
    let e = TestEnv::default();
    let f = e.dir().join("contract.wasm");
    let cmd = e.cmd_arr::<fetch::Cmd>(&[
        "--id",
        "bc074f0f03934d0189653bc15af9a83170411e103b4c48a63888306cfba41ac8",
        "--network",
        "futurenet",
        "--out-file",
        f.to_str().unwrap(),
    ]);
    cmd.run().await.unwrap();
    assert!(f.exists());
}
