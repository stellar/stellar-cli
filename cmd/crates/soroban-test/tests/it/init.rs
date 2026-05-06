use assert_fs::prelude::*;
use predicates::prelude::predicate;
use soroban_test::TestEnv;

#[test]
fn init() {
    let sandbox = TestEnv::default();

    // Get the CLI version
    let cli_version = soroban_cli::commands::version::pkg();
    let cli_is_rc = cli_version.contains("rc");

    // Get the SDK version the CLI depends on
    let root_cargo_toml = std::fs::read_to_string("../../../Cargo.toml").unwrap();
    let root_table = toml::from_str::<toml::Table>(&root_cargo_toml).unwrap();
    let cli_sdk_version = root_table["workspace"]["dependencies"]["soroban-sdk"]["version"]
        .as_str()
        .unwrap();
    let cli_sdk_major: u32 = cli_sdk_version.split('.').next().unwrap().parse().unwrap();
    let sdk_is_rc = cli_sdk_version.contains("rc");

    // Target version str of the initialized project
    // for CLI's released with RC sdk versions, init should use the previous major version
    let target_sdk_major = if sdk_is_rc {
        if cli_sdk_major == 25 {
            // there is no soroban-sdk 24, so we special case this
            23
        } else {
            cli_sdk_major - 1
        }
    } else {
        cli_sdk_major
    };
    let target_sdk_major_str = target_sdk_major.to_string();

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
            let sdk_version = table["workspace"]["dependencies"]["soroban-sdk"].as_str();
            println!("Check expected version {target_sdk_major_str} based on CLI's SDK version {cli_sdk_version} matches template's {sdk_version:?}");
            if cli_is_rc {
                sdk_version.and_then(|x| x.split('-').next()) == Some(target_sdk_major_str.as_str())
            } else {
                sdk_version == Some(target_sdk_major_str.as_str())
            }
        }));
}
