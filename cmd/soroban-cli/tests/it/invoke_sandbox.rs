use soroban_test::TestEnv;

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
        .arg("--identity")
        .arg("test_id")
        .arg("--id")
        .arg(id)
        .arg("--fn=hello")
        .arg("--")
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
        .arg("--fn=hello")
        .arg("--")
        .arg("--world=world")
        .assert()
        .stdout("[\"Hello\",\"world\"]\n")
        .success();
}

#[test]
fn invoke_respects_conflicting_args() {
    let sandbox = TestEnv::default();
    sandbox
        .new_cmd("contract")
        .arg("invoke")
        .arg("--id=1")
        .arg("--identity")
        .arg("test")
        .arg("--account")
        .arg("GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF")
        .arg("--wasm")
        .arg(HELLO_WORLD.path())
        .arg("--fn=hello")
        .arg("--")
        .arg("--world=world")
        .assert()
        .stderr(predicates::str::contains(
            "The argument \'--identity <IDENTITY>\' cannot be used with \'--account <ACCOUNT_ID>\'",
        ))
        .failure();

    sandbox
        .new_cmd("contract")
        .arg("invoke")
        .arg("--id=1")
        .arg("--rpc-url")
        .arg("localhost:8000")
        .arg("--account")
        .arg("GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF")
        .arg("--wasm")
        .arg(HELLO_WORLD.path())
        .arg("--fn=hello")
        .arg("--")
        .arg("--world=world")
        .assert()
        .stderr(predicates::str::contains(
            "The argument \'--rpc-url <RPC_URL>\' cannot be used with \'--account <ACCOUNT_ID>\'",
        ))
        .failure();
}

#[test]
fn invoke_auth() {
    TestEnv::with_default(|sandbox| {
        sandbox
            .new_cmd("contract")
            .arg("invoke")
            .arg("--account")
            .arg("GD5KD2KEZJIGTC63IGW6UMUSMVUVG5IHG64HUTFWCHVZH2N2IBOQN7PS")
            .arg("--id=1")
            .arg("--wasm")
            .arg(HELLO_WORLD.path())
            .arg("--fn=auth")
            .arg("--")
            .arg("--addr=GD5KD2KEZJIGTC63IGW6UMUSMVUVG5IHG64HUTFWCHVZH2N2IBOQN7PS")
            .arg("--world=world")
            .assert()
            .stdout("[\"Hello\",\"world\"]\n")
            .success();
    });
}

#[test]
fn invoke_hello_world_with_seed() {
    let sandbox = TestEnv::default();
    let identity = add_test_seed(sandbox.dir());
    sandbox
        .new_cmd("contract")
        .arg("invoke")
        .arg("--identity")
        .arg(identity)
        .arg("--id=1")
        .arg("--wasm")
        .arg(HELLO_WORLD.path())
        .arg("--fn=hello")
        .arg("--")
        .arg("--world=world")
        .assert()
        .stdout("[\"Hello\",\"world\"]\n")
        .success();
}
