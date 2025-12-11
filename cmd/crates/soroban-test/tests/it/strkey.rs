use predicates::prelude::predicate;
use soroban_test::TestEnv;

#[test]
fn decode() {
    let sandbox = TestEnv::default();
    sandbox
        .new_assert_cmd("strkey")
        .args([
            "decode",
            "GAKSH6AD2IPJQELTHIOWDAPYX74YELUOWJLI2L4RIPIPZH6YQIFNUSDC",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("public_key_ed25519"));
}

#[test]
fn encode() {
    let sandbox = TestEnv::default();
    sandbox
        .new_assert_cmd("strkey")
        .args(["encode", r#"{"public_key_ed25519":"1523f803d21e9811733a1d6181f8bff9822e8eb2568d2f9143d0fc9fd8820ada"}"#])
        .assert()
        .success()
        .stdout(predicate::str::contains("GAKSH6AD2IPJQELTHIOWDAPYX74YELUOWJLI2L4RIPIPZH6YQIFNUSDC"));
}
