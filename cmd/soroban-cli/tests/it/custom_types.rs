use std::fmt::Display;

use assert_cmd::Command;
use serde_json::json;

use crate::util::{temp_ledger_file, test_wasm, CommandExt, Sandbox, SorobanCommand};

fn invoke(func: &str) -> Command {
    let mut s = Sandbox::new_cmd("invoke");
    s.arg("--ledger-file")
        .arg(temp_ledger_file())
        .arg("--id=1")
        .arg("--wasm")
        .arg(test_wasm("test_custom_types"))
        .arg("--fn")
        .arg(func)
        .arg("--");
    s
}

fn invoke_with_roundtrip<D>(func: &str, data: D)
where
    D: Display,
{
    invoke(func)
        .arg(&format!("--{func}"))
        .json_arg(&data)
        .assert()
        .success()
        .stdout(format!("{data}\n"));
}

#[test]
fn symbol() {
    invoke("hello")
        .arg("--hello")
        .arg("world")
        .assert()
        .success()
        .stdout(
            r#""world"
"#,
        );
}

#[test]
fn symbol_with_quotes() {
    invoke_with_roundtrip("hello", json!("world"));
}

#[test]
fn generate_help() {
    invoke("test").arg("--help").assert().success();
}

#[test]
fn strukt() {
    invoke_with_roundtrip("strukt", json!({"a": 42, "b": true, "c": "world"}));
}

#[test]
fn enum_2_str() {
    invoke_with_roundtrip("simple", json!("First"));
}

#[test]
fn e_2_s_enum() {
    invoke_with_roundtrip("complex", json!({"Enum": "First"}));
}

#[test]
fn e_2_s_tuple() {
    invoke_with_roundtrip(
        "complex",
        json!({"Tuple": [{"a": 42, "b": true, "c": "world"}, "First"]}),
    );
}

#[test]
fn e_2_s_strukt() {
    invoke_with_roundtrip(
        "complex",
        json!({"Struct": {"a": 42, "b": true, "c": "world"}}),
    );
}

#[test]
fn number_arg() {
    invoke_with_roundtrip("u32_", 42);
}

#[test]
fn account() {
    invoke_with_roundtrip(
        "account",
        json!("GD5KD2KEZJIGTC63IGW6UMUSMVUVG5IHG64HUTFWCHVZH2N2IBOQN7PS"),
    );
}

#[test]
fn bytes() {
    invoke_with_roundtrip("bytes", json!("7374656c6c6172"));
}

#[test]
fn const_enum() {
    invoke_with_roundtrip("card", "11");
}

#[test]
fn boolean() {
    invoke("boolean")
        .arg("--boolean")
        .assert()
        .success()
        .stdout(
            r#"true
"#,
        );
}
#[test]
fn boolean_no_flag() {
    invoke("boolean").assert().success().stdout(
        r#"false
"#,
    );
}

#[test]
fn boolean_not() {
    invoke("not").arg("--boolean").assert().success().stdout(
        r#"false
"#,
    );
}

#[test]
fn boolean_not_no_flag() {
    invoke("not").assert().success().stdout(
        r#"true
"#,
    );
}
