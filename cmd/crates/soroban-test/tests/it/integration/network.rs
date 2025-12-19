use predicates::prelude::{predicate, PredicateBooleanExt};
use soroban_test::TestEnv;

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
