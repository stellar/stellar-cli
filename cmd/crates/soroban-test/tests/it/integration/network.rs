use predicates::prelude::{predicate, PredicateBooleanExt};
use soroban_test::{AssertExt, TestEnv};

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn set_default_network() {
    let sandbox = &TestEnv::new();

    sandbox
        .new_assert_cmd("network")
        .arg("use")
        .arg("testnet")
        .assert()
        .stderr(predicate::str::contains(
            "The default network is set to `testnet`",
        ))
        .success();
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn unset_default_network() {
    let sandbox = &TestEnv::new();

    sandbox
        .new_assert_cmd("network")
        .arg("use")
        .arg("testnet")
        .assert()
        .success();

    sandbox
        .new_assert_cmd("env")
        .env_remove("STELLAR_NETWORK")
        .assert()
        .stdout(predicate::str::contains("STELLAR_NETWORK=testnet"))
        .success();

    sandbox
        .new_assert_cmd("network")
        .arg("unset")
        .assert()
        .stderr(predicate::str::contains(
            "The default network has been unset",
        ))
        .success();

    sandbox
        .new_assert_cmd("env")
        .env_remove("STELLAR_NETWORK")
        .assert()
        .stdout(predicate::str::contains("STELLAR_NETWORK=").not())
        .success();
}

#[tokio::test]
async fn network_info_includes_id_in_text_output() {
    let sandbox = &TestEnv::new();
    sandbox
        .new_assert_cmd("network")
        .arg("info")
        .assert()
        .success()
        .stderr(predicate::str::contains(
            "Network Id: baefd734b8d3e48472cff83912375fedbc7573701912fe308af730180f97d74a",
        ));
}

#[tokio::test]
async fn network_info_includes_id_in_json_output() {
    let sandbox = &TestEnv::new();
    let output = sandbox
        .new_assert_cmd("network")
        .arg("info")
        .arg("--output")
        .arg("json")
        .assert()
        .success()
        .stdout_as_str();
    let info: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(
        info["id"],
        "baefd734b8d3e48472cff83912375fedbc7573701912fe308af730180f97d74a"
    );
}
