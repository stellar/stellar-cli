use std::path::PathBuf;

use soroban_test::TestEnv;

use crate::util::{invoke_custom as invoke, CUSTOM_TYPES};

fn invoke_custom(e: &TestEnv, func: &str) -> assert_cmd::Command {
    invoke(e, "1", func, [PathBuf::from("--wasm"), CUSTOM_TYPES.path()])
}

#[test]
fn generate_help() {
    invoke_custom(&TestEnv::default(), "strukt_hel")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "Example contract method which takes a struct",
        ));
}
#[test]
fn vec_help() {
    invoke_custom(&TestEnv::default(), "vec")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicates::str::contains("Array<u32>"));
}

#[test]
fn tuple_help() {
    invoke_custom(&TestEnv::default(), "tuple")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicates::str::contains("Tuple<Symbol, u32>"));
}

#[test]
fn strukt_help() {
    invoke_custom(&TestEnv::default(), "strukt")
        .arg("--help")
        .assert()
        .stdout(predicates::str::contains(
            "--strukt '{ \"a\": 1, \"b\": true, \"c\": \"hello\" }'",
        ))
        .stdout(predicates::str::contains(
            "This is from the rust doc above the struct Test",
        ));
}

#[test]
fn complex_enum_help() {
    invoke_custom(&TestEnv::default(), "complex")
        .arg("--help")
        .assert()
        .stdout(predicates::str::contains(
            r#"--complex '{"Struct":{ "a": 1, "b": true, "c": "hello" }}"#,
        ))
        .stdout(predicates::str::contains(
            r#"{"Tuple":[{ "a": 1, "b": true, "c": "hello" }"#,
        ))
        .stdout(predicates::str::contains(
            r#"{"Enum":"First"|"Second"|"Third"}"#,
        ))
        .stdout(predicates::str::contains(
            r#"{"Asset":["GDIY6AQQ75WMD4W46EYB7O6UYMHOCGQHLAQGQTKHDX4J2DYQCHVCR4W4", "-100"]}"#,
        ))
        .stdout(predicates::str::contains(r#""Void"'"#));
}

#[test]
fn multi_arg_failure() {
    invoke_custom(&TestEnv::default(), "multi_args")
        .arg("--b")
        .assert()
        .failure()
        .stderr("error: Missing argument a\n");
}

#[test]
fn handle_arg_larger_than_i32_failure() {
    invoke_custom(&TestEnv::default(), "i32_")
        .arg("--i32_")
        .arg(u32::MAX.to_string())
        .assert()
        .failure()
        .stderr(predicates::str::contains("value is not parseable"));
}

#[test]
fn handle_arg_larger_than_i64_failure() {
    invoke_custom(&TestEnv::default(), "i64_")
        .arg("--i64_")
        .arg(u64::MAX.to_string())
        .assert()
        .failure()
        .stderr(predicates::str::contains("value is not parseable"));
}

#[test]
fn build() {
    let sandbox = TestEnv::default();
    let cargo_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let hello_world_contract_path =
        cargo_dir.join("tests/fixtures/test-wasms/hello_world/Cargo.toml");
    sandbox
        .new_assert_cmd("contract")
        .arg("build")
        .arg("--manifest-path")
        .arg(hello_world_contract_path)
        .arg("--profile")
        .arg("test-wasms")
        .arg("--package")
        .arg("test_hello_world")
        .assert()
        .success();
}
