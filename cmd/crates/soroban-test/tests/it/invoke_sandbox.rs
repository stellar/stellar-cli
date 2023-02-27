use soroban_cli::commands::contract::invoke;
use soroban_test::TestEnv;
use crate::util::{
    add_test_seed, Sandbox, DEFAULT_PUB_KEY, DEFAULT_PUB_KEY_1, DEFAULT_SECRET_KEY,
    DEFAULT_SEED_PHRASE, HELLO_WORLD,
};

use crate::util::{add_test_seed, HELLO_WORLD};

#[test]
fn install_wasm_then_deploy_contract() {
    let hash = HELLO_WORLD.hash().unwrap();
    let sandbox = TestEnv::default();
    sandbox
        .new_cmd("contract")
        .arg("install")
        .arg("--wasm")
        .arg(HELLO_WORLD.path())
        .assert()
        .success()
        .stdout(format!("{hash}\n"));

    sandbox
        .new_cmd("contract")
        .arg("deploy")
        .arg("--wasm-hash")
        .arg(&format!("{hash}"))
        .arg("--id=1")
        .assert()
        .success()
        .stdout("0000000000000000000000000000000000000000000000000000000000000001\n");
}

#[test]
fn deploy_contract_with_wasm_file() {
    TestEnv::default()
        .new_cmd("contract")
        .arg("deploy")
        .arg("--wasm")
        .arg(HELLO_WORLD.path())
        .arg("--id=1")
        .assert()
        .success()
        .stdout("0000000000000000000000000000000000000000000000000000000000000001\n");
}

#[test]
fn invoke_hello_world_with_deploy_first() {
    let sandbox = TestEnv::default();
    let res = sandbox
        .new_cmd("contract")
        .arg("deploy")
        .arg("--wasm")
        .arg(HELLO_WORLD.path())
        .assert()
        .success();
    let stdout = String::from_utf8(res.get_output().stdout.clone()).unwrap();
    let id = stdout.trim_end();
    sandbox
        .new_cmd("contract")
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
        .new_cmd("contract")
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
<<<<<<< refs/remotes/stellar/main:cmd/soroban-cli/tests/it/invoke_sandbox.rs
fn invoke_auth() {
    let sandbox = Sandbox::new();
=======
fn invoke_hello_world_with_lib() {
    let sandbox = TestEnv::default();
    let res = invoke::Cmd {
        contract_id: "1".to_string(),
        wasm: Some(HELLO_WORLD.path()),
        function: "hello".to_string(),
        slop: vec!["--world=world".into()],
        ..Default::default()
    };
    std::env::set_current_dir(sandbox.dir()).unwrap();
    assert_eq!(res.run_in_sandbox().unwrap(), r#"["Hello","world"]"#);
}

#[test]
fn invoke_hello_world_with_lib_two() {
    let sandbox = TestEnv::default();

    let cmd: invoke::Cmd = format!(
        "invoke --id=1 --wasm {} --fn=hello -- --world=world",
        HELLO_WORLD.path().display()
    )
    .parse()
    .unwrap();
    std::env::set_current_dir(sandbox.dir()).unwrap();
    assert_eq!(cmd.run_in_sandbox().unwrap(), r#"["Hello","world"]"#);
}
// #[test]
// fn invoke_hello_world_with_lib_three() {
//     let sandbox = TestEnv::default();
//     let builder  = invoke::CmdBuilder::new().contract_id("1").wasm(HELLO_WORLD.path()).function("hello").slop(["--hello=world"]).build();
//     std::env::set_current_dir(sandbox.dir()).unwrap();
//     assert_eq!(res.run_in_sandbox().unwrap(), r#"["Hello","world"]"#);
// }

#[test]
fn invoke_respects_conflicting_args() {
    let sandbox = TestEnv::default();
>>>>>>> feat: use FromStr to add parse method:cmd/crates/soroban-test/tests/it/invoke_sandbox.rs
    sandbox
        .new_cmd("contract")
        .arg("invoke")
        .arg("--id=1")
        .arg("--wasm")
        .arg(HELLO_WORLD.path())
        .arg("--")
        .arg("auth")
        .arg(&format!("--addr={DEFAULT_PUB_KEY}"))
        .arg("--world=world")
        .assert()
        .stdout("[\"Hello\",\"world\"]\n")
        .success();
}

#[test]
fn invoke_auth_with_different_test_account() {
    let sandbox = Sandbox::new();
    sandbox
        .new_cmd("contract")
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
        .stdout("[\"Hello\",\"world\"]\n")
        .success();
}

#[test]
fn invoke_auth_with_different_test_account_fail() {
    let sandbox = Sandbox::new();
    sandbox
        .new_cmd("contract")
        .arg("invoke")
        .arg("--hd-path=1")
        .arg("--id=1")
        .arg("--wasm")
        .arg(HELLO_WORLD.path())
        .arg("--")
        .arg("auth")
        .arg(&format!("--addr={DEFAULT_PUB_KEY}"))
        .arg("--world=world")
        .assert()
        .success()
        .stdout("")
        .stderr(predicates::str::contains("HostError"));
}

#[test]
fn invoke_hello_world_with_seed() {
    let sandbox = TestEnv::default();
    let identity = add_test_seed(sandbox.dir());
<<<<<<< refs/remotes/stellar/main:cmd/soroban-cli/tests/it/invoke_sandbox.rs
    invoke_with_source(&sandbox, &identity);
}

#[test]
fn invoke_with_seed() {
    let sandbox = Sandbox::new();
    invoke_with_source(&sandbox, DEFAULT_SEED_PHRASE);
}

#[test]
fn invoke_with_id() {
    let sandbox = Sandbox::new();
    let identity = add_test_seed(sandbox.dir());
    invoke_with_source(&sandbox, &identity);
}

#[test]
fn invoke_with_sk() {
    let sandbox = Sandbox::new();
    invoke_with_source(&sandbox, DEFAULT_SECRET_KEY);
}

fn invoke_with_source(sandbox: &Sandbox, source: &str) {
=======
    let path = HELLO_WORLD.path();
>>>>>>> feat: use FromStr to add parse method:cmd/crates/soroban-test/tests/it/invoke_sandbox.rs
    sandbox
        .new_cmd("contract")
        .arg("invoke")
        .arg("--source-account")
        .arg(source)
        .arg("--id=1")
        .arg("--wasm")
<<<<<<< main:cmd/soroban-cli/tests/it/invoke_sandbox.rs
        .arg(HELLO_WORLD.path())
=======
        .arg(path)
        .arg("--fn=hello")
>>>>>>> feat: use FromStr to add parse method:cmd/crates/soroban-test/tests/it/invoke_sandbox.rs
        .arg("--")
        .arg("hello")
        .arg("--world=world")
        .assert()
        .stdout("[\"Hello\",\"world\"]\n")
        .success();
}
