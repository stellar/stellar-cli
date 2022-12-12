use assert_cmd::Command;
use serde_json::json;

use crate::util::{temp_ledger_file, test_wasm, CommandUtil, Sandbox, SorobanCommand};

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

#[test]
fn symbol() {
    invoke("hello")
        .arg("--world")
        .arg("world")
        .assert()
        .success()
        .stdout(
            r#"["Hello","world"]
"#,
        );
}

#[test]
fn symbol_with_quotes() {
    invoke("hello")
        .arg("--world")
        .json_arg(json!("world"))
        .assert()
        .success()
        .stdout(
            r#"["Hello","world"]
"#,
        );
}


#[test]
fn generate_help() {
    invoke("test").arg("--help").assert().success();
}

#[test]
fn strukt() {
    invoke("strukt")
        .arg("--strukt")
        .json_arg(json!({"a": 42, "b": true, "c": "world"}))
        .assert()
        .success()
        .stdout(
            r#"["Hello","world"]
"#,
        );
}

#[test]
fn enum_2_str() {
    invoke("enum_2_str")
        .arg("--simple")
        .arg("First")
        .assert()
        .success()
        .stdout(
            r#"[["First"]]
"#,
        );
}

#[test]
fn e_2_s_enum() {
    invoke("e_2_s")
        .arg("--complex")
        .json_arg(json!({"Enum": "First"}))
        .assert()
        .success()
        .stdout(
            r#"[["Enum",["First"]]]
"#,
        );
}

#[test]
fn e_2_s_tuple() {
    invoke("e_2_s")
        .arg("--complex")
        .json_arg(json!({"Tuple": [{"a": 42, "b": true, "c": "world"}, "First"]}))
        .assert()
        .success()
        .stdout(
            r#"[["Tuple",[{"a":42,"b":true,"c":"world"},["First"]]]]
"#,
        );
}

#[test]
fn e_2_s_strukt() {
    invoke("e_2_s")
        .arg("--complex")
        .json_arg(json!({"Struct": {"a": 42, "b": true, "c": "world"}}))
        .assert()
        .success()
        .stdout(
            r#"[["Struct",{"a":42,"b":true,"c":"world"}]]
"#,
        );
}


#[test]
fn number_arg() {
    invoke("u32_")
        .arg("--u32_")
        .arg("42")
        .assert()
        .success()
        .stdout(
            r#"[42]
"#,
        );
}

#[test]
fn account() {
    invoke("account")
        .arg("--account_id")
        .arg("GD5KD2KEZJIGTC63IGW6UMUSMVUVG5IHG64HUTFWCHVZH2N2IBOQN7PS")
        .assert()
        .success()
        .stdout(
            r#"["GD5KD2KEZJIGTC63IGW6UMUSMVUVG5IHG64HUTFWCHVZH2N2IBOQN7PS"]
"#,
        );
}

#[test]
fn bytes() {
    invoke("bytes")
        .arg("--bytes")
        .arg("7374656c6c6172")
        .assert()
        .success()
        .stdout(
            r#"[[115,116,101,108,108,97,114]]
"#,
        );
}

#[test]
fn const_enum() {
    invoke("card")
        .arg("--card")
        .arg("11")
        .assert()
        .success()
        .stdout(
            r#"[11]
"#,
        );
}

#[test]
fn boolean() {
    invoke("not")
        .arg("--boolean")
        .assert()
        .success()
        .stdout(
            r#"[false]
"#,
        );
}

#[test]
fn boolean_no_flag() {
    invoke("not")
        .assert()
        .success()
        .stdout(
            r#"[true]
"#,
        );
}
