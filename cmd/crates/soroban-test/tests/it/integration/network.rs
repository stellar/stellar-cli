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
async fn network_ls_long_conceals_rpc_headers() {
    let sandbox = &TestEnv::new();

    sandbox
        .new_assert_cmd("network")
        .args([
            "add",
            "--rpc-url",
            "http://localhost:8000/rpc",
            "--network-passphrase",
            "Test Network",
            "--rpc-header",
            "Authorization: Bearer secret123",
            "--rpc-header",
            "X-Api-Key: mykey",
            "test-concealed",
        ])
        .assert()
        .success();

    sandbox
        .new_assert_cmd("network")
        .args(["ls", "--long"])
        .assert()
        .stdout(predicate::str::contains(
            "Name: test-concealed\nRPC url: http://localhost:8000/rpc\nRPC headers:\n  Authorization: <concealed>\n  X-Api-Key: <concealed>",
        ))
        .stdout(predicate::str::contains("Bearer secret123").not())
        .stdout(predicate::str::contains("mykey").not())
        .success();
}

#[tokio::test]
async fn network_ls_long_shows_not_set_when_no_rpc_headers() {
    let sandbox = &TestEnv::new();

    sandbox
        .new_assert_cmd("network")
        .args([
            "add",
            "--rpc-url",
            "http://localhost:8000/rpc",
            "--network-passphrase",
            "Test Network",
            "test-no-headers",
        ])
        .assert()
        .success();

    sandbox
        .new_assert_cmd("network")
        .args(["ls", "--long"])
        .assert()
        .stdout(predicate::str::contains(
            "Name: test-no-headers\nRPC url: http://localhost:8000/rpc\nRPC headers: not set",
        ))
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
