use soroban_test::TestEnv;

use super::util::deploy_hello;

fn write_env_file(e: &TestEnv, contents: &str) {
    let env_file = e.dir().join(".env");
    let contents = format!("SOROBAN_CONTRACT_ID={contents}");
    std::fs::write(&env_file, &contents).unwrap();
    assert_eq!(contents, std::fs::read_to_string(env_file).unwrap());
}

#[tokio::test]
async fn can_read_file() {
    let e = &TestEnv::default();
    std::thread::sleep(core::time::Duration::from_millis(1000));
    let id = deploy_hello(e).await;
    println!("{id}");
    write_env_file(e, &id);
    e.new_assert_cmd("contract")
        .arg("invoke")
        .arg("--")
        .arg("hello")
        .arg("--world=world")
        .assert()
        .stdout("[\"Hello\",\"world\"]\n")
        .success();
}

#[tokio::test]
async fn current_env_not_overwritten() {
    let e = TestEnv::default();
    std::thread::sleep(core::time::Duration::from_millis(3000));
    write_env_file(&e, &deploy_hello(&e).await);
    e.new_assert_cmd("contract")
        .env(
            "SOROBAN_CONTRACT_ID",
            "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAFCT4",
        )
        .arg("invoke")
        .arg("--")
        .arg("hello")
        .arg("--world=world")
        .assert()
        .stderr(
            "❌ error: Contract not found: CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAFCT4\n",
        );
}

#[tokio::test]
async fn cli_args_have_priority() {
    let e = &TestEnv::default();
    std::thread::sleep(core::time::Duration::from_millis(6000));
    let id = deploy_hello(e).await;
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
