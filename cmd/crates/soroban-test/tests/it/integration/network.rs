use predicates::prelude::{predicate, PredicateBooleanExt};
use soroban_test::TestEnv;

#[tokio::test]
async fn network_id() {
    let sandbox = &TestEnv::new();

    sandbox
        .new_assert_cmd("network")
        .arg("id")
        .arg("--network")
        .arg("testnet")
        .assert()
        .stdout(predicate::str::starts_with(
            "cee0302d59844d32bdca915c8203dd44b33fbb7edc19051ea37abedf28ecd472",
        ))
        .success();
}

#[tokio::test]
async fn network_id_json() {
    let sandbox = &TestEnv::new();

    sandbox
        .new_assert_cmd("network")
        .arg("id")
        .arg("--network")
        .arg("testnet")
        .arg("--output")
        .arg("json")
        .assert()
        .stdout(
            predicate::str::contains(
                "\"id\":\"cee0302d59844d32bdca915c8203dd44b33fbb7edc19051ea37abedf28ecd472\"",
            )
            .and(predicate::str::contains(
                "\"network_passphrase\":\"Test SDF Network ; September 2015\"",
            )),
        )
        .success();
}

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
