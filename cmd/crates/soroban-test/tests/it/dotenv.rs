use soroban_test::TestEnv;

use crate::util::HELLO_WORLD;

const SOROBAN_CONTRACT_ID: &str = "SOROBAN_CONTRACT_ID=1";

fn deploy(e: &TestEnv, id: &str) {
    e.new_assert_cmd("contract")
        .arg("deploy")
        .arg("--wasm")
        .arg(HELLO_WORLD.path())
        .arg("--id")
        .arg(id)
        .assert()
        .success();
}

fn write_env_file(e: &TestEnv, contents: &str) {
    let env_file = e.dir().join(".env");
    std::fs::write(&env_file, contents).unwrap();
    assert_eq!(contents, std::fs::read_to_string(env_file).unwrap());
}

#[test]
fn can_read_file() {
    TestEnv::with_default(|e| {
        deploy(e, "1");
        write_env_file(e, SOROBAN_CONTRACT_ID);
        e.new_assert_cmd("contract")
            .arg("invoke")
            .arg("--")
            .arg("hello")
            .arg("--world=world")
            .assert()
            .stdout("[\"Hello\",\"world\"]\n")
            .success();
    });
}

#[test]
fn current_env_not_overwritten() {
    TestEnv::with_default(|e| {
        deploy(e, "1");
        write_env_file(e, SOROBAN_CONTRACT_ID);

        e.new_assert_cmd("contract")
            .env("SOROBAN_CONTRACT_ID", "2")
            .arg("invoke")
            .arg("--")
            .arg("hello")
            .arg("--world=world")
            .assert()
            .stderr("error: parsing contract spec: contract spec not found\n");
    });
}

#[test]
fn cli_args_have_priority() {
    TestEnv::with_default(|e| {
        deploy(e, "2");
        write_env_file(e, SOROBAN_CONTRACT_ID);
        e.new_assert_cmd("contract")
            .env("SOROBAN_CONTRACT_ID", "3")
            .arg("invoke")
            .arg("--id")
            .arg("2")
            .arg("--")
            .arg("hello")
            .arg("--world=world")
            .assert()
            .stdout("[\"Hello\",\"world\"]\n");
    });
}
