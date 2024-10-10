use assert_fs::prelude::*;
use predicates::prelude::predicate;
use soroban_test::TestEnv;

#[test]
fn init() {
    let sandbox = TestEnv::default();
    let major = soroban_cli::commands::version::pkg()
        .split('.')
        .next()
        .unwrap();
    sandbox
        .new_assert_cmd("contract")
        .arg("init")
        .arg(".")
        .assert()
        .success();
    sandbox
        .dir()
        .child("Cargo.toml")
        .assert(predicate::function(|c: &str| {
            let table = toml::from_str::<toml::Table>(c).unwrap();
            table["workspace"]["dependencies"]["soroban-sdk"].as_str()
                == Some(&format!("{major}.0.0"))
        }));
}
