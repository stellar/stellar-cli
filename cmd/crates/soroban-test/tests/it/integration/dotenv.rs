use soroban_test::TestEnv;

use super::util::deploy_hello;

const SOROBAN_FEE: &str = "100";

fn write_env_file(e: &TestEnv, contents: &str) {
    let env_file = e.dir().join(".env");
    let contents = format!("SOROBAN_CONTRACT_ID={contents}");
    std::fs::write(&env_file, &contents).unwrap();
    assert_eq!(contents, std::fs::read_to_string(env_file).unwrap());
}

#[test]
fn can_read_file() {
    let e = &TestEnv::new();
    let id = deploy_hello(e);
    write_env_file(e, &id);
    e.new_assert_cmd("contract")
        .env("SOROBAN_FEE", SOROBAN_FEE)
        .arg("invoke")
        .arg("--")
        .arg("hello")
        .arg("--world=world")
        .assert()
        .stdout("[\"Hello\",\"world\"]\n")
        .success();
}

#[test]
fn current_env_not_overwritten() {
    let e = TestEnv::new();
    write_env_file(&e, &deploy_hello(&e));

    e.new_assert_cmd("contract")
        .env(
            "SOROBAN_CONTRACT_ID",
            "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAFCT4",
        )
        .env("SOROBAN_FEE", SOROBAN_FEE)
        .arg("invoke")
        .arg("--")
        .arg("hello")
        .arg("--world=world")
        .assert()
        .stderr(
            "error: Contract not found: CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAFCT4\n",
        );
}

#[test]
fn cli_args_have_priority() {
    let e = &TestEnv::new();
    let id = deploy_hello(e);
    write_env_file(e, &id);
    e.new_assert_cmd("contract")
        .env(
            "SOROBAN_CONTRACT_ID",
            "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAFCT4",
        )
        .arg("invoke")
        .arg("--id")
        .arg(id)
        .arg("--")
        .arg("hello")
        .arg("--world=world")
        .assert()
        .stdout("[\"Hello\",\"world\"]\n");
}
