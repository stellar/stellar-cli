use serde_json::json;

use crate::util::{invoke, invoke_with_roundtrip, Sandbox};

#[test]
fn symbol() {
    invoke(&Sandbox::new(), "hello")
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
    invoke(&Sandbox::new(), "test")
        .arg("--help")
        .assert()
        .success();
}

#[test]
fn multi_arg() {
    invoke(&Sandbox::new(), "multi_args")
        .arg("--b")
        .assert()
        .success()
        .stderr("error: Missing argument a\n");
}

#[test]
fn multi_arg_success() {
    invoke(&Sandbox::new(), "multi_args")
        .arg("--a")
        .arg("42")
        .arg("--b")
        .assert()
        .success()
        .stdout("42\n");
}

#[test]
fn map() {
    invoke_with_roundtrip("map", json!({"0": true, "1": false}));
}

#[test]
fn map_help() {
    invoke(&Sandbox::new(), "map")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicates::str::contains("map<u32, bool>"));
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
fn account_address() {
    invoke_with_roundtrip(
        "address",
        json!("GD5KD2KEZJIGTC63IGW6UMUSMVUVG5IHG64HUTFWCHVZH2N2IBOQN7PS"),
    );
}

#[test]
fn contract_address() {
    invoke_with_roundtrip(
        "address",
        json!("CA3D5KRYM6CB7OWQ6TWYRR3Z4T7GNZLKERYNZGGA5SOAOPIFY6YQGAXE"),
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
fn parse_u128() {
    let num = "340000000000000000000000000000000000000";
    invoke(&Sandbox::new(), "u128")
        .arg("--u128")
        .arg(num)
        .assert()
        .success()
        .stdout(format!(
            r#""{num}"
"#,
        ));
}

#[test]
fn parse_i128() {
    let num = "170000000000000000000000000000000000000";
    invoke(&Sandbox::new(), "i128")
        .arg("--i128")
        .arg(num)
        .assert()
        .success()
        .stdout(format!(
            r#""{num}"
"#,
        ));
}

// TODO: support more ways to enter i128s
// #[test]
// fn parse_negative_i128() {
//     let num = "-170000000000000000000000000000000000000";
//     invoke(&Sandbox::new(), "i128")
//         .arg("--i128")
//         .arg(num)
//         .assert()
//         .success()
//         .stdout(format!(
//             r#""{num}"
// "#,
//         ));
// }
//
// #[test]
// fn parse_quoted_i128() {
//     let num = "\"-170000000000000000000000000000000000000\"";
//     invoke(&Sandbox::new(), "i128")
//         .arg("--i128")
//         .arg(num)
//         .assert()
//         .success()
//         .stdout(format!(
//             r#""{num}"
// "#,
//         ));
// }

#[test]
fn boolean() {
    invoke(&Sandbox::new(), "boolean")
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
    invoke(&Sandbox::new(), "boolean")
        .assert()
        .success()
        .stdout(
            r#"false
"#,
        );
}

#[test]
fn boolean_not() {
    invoke(&Sandbox::new(), "not")
        .arg("--boolean")
        .assert()
        .success()
        .stdout(
            r#"false
"#,
        );
}

#[test]
fn boolean_not_no_flag() {
    invoke(&Sandbox::new(), "not").assert().success().stdout(
        r#"true
"#,
    );
}
