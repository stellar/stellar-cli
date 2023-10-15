use serde_json::json;

use soroban_cli::commands;
use soroban_test::TestEnv;

use super::util::{invoke, invoke_with_roundtrip, CUSTOM_TYPES};

#[test]
fn symbol() {
    invoke(&TestEnv::default(), "hello")
        .arg("--hello")
        .arg("world")
        .assert()
        .success()
        .stdout(
            r#""world"
"#,
        );
}

#[tokio::test]
async fn string_with_quotes() {
    invoke_with_roundtrip("string", json!("hello world")).await;
}

#[tokio::test]
async fn symbol_with_quotes() {
    invoke_with_roundtrip("hello", json!("world")).await;
}

#[test]
fn generate_help() {
    invoke(&TestEnv::default(), "strukt_hel")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "Example contract method which takes a struct",
        ));
}

#[test]
fn multi_arg_failure() {
    invoke(&TestEnv::default(), "multi_args")
        .arg("--b")
        .assert()
        .failure()
        .stderr("error: Missing argument a\n");
}

#[test]
fn multi_arg_success() {
    invoke(&TestEnv::default(), "multi_args")
        .arg("--a")
        .arg("42")
        .arg("--b")
        .assert()
        .success()
        .stdout("42\n");
}

#[test]
fn bytes_as_file() {
    let env = &TestEnv::default();
    let path = env.temp_dir.join("bytes.txt");
    std::fs::write(&path, 0x0073_7465_6c6c_6172u128.to_be_bytes()).unwrap();
    invoke(env, "bytes")
        .arg("--bytes-file-path")
        .arg(path)
        .assert()
        .success()
        .stdout("\"0000000000000000007374656c6c6172\"\n");
}

#[tokio::test]
async fn map() {
    invoke_with_roundtrip("map", json!({"0": true, "1": false})).await;
}

#[test]
fn map_help() {
    invoke(&TestEnv::default(), "map")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicates::str::contains("Map<u32, bool>"));
}

#[tokio::test]
async fn vec_() {
    invoke_with_roundtrip("vec", json!([0, 1])).await;
}

#[test]
fn vec_help() {
    invoke(&TestEnv::default(), "vec")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicates::str::contains("Array<u32>"));
}

#[tokio::test]
async fn tuple() {
    invoke_with_roundtrip("tuple", json!(["hello", 0])).await;
}

#[tokio::test]
async fn tuple_help() {
    invoke(&TestEnv::default(), "tuple")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicates::str::contains("Tuple<Symbol, u32>"));
}

#[tokio::test]
async fn strukt() {
    invoke_with_roundtrip("strukt", json!({"a": 42, "b": true, "c": "world"})).await;
}

#[tokio::test]
async fn tuple_strukt() {
    invoke_with_roundtrip(
        "tuple_strukt",
        json!([{"a": 42, "b": true, "c": "world"}, "First"]),
    )
    .await;
}

#[test]
fn strukt_help() {
    invoke(&TestEnv::default(), "strukt")
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
    invoke(&TestEnv::default(), "complex")
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

#[tokio::test]
async fn enum_2_str() {
    invoke_with_roundtrip("simple", json!("First")).await;
}

#[tokio::test]
async fn e_2_s_enum() {
    invoke_with_roundtrip("complex", json!({"Enum": "First"})).await;
}

#[tokio::test]
async fn asset() {
    invoke_with_roundtrip(
        "complex",
        json!({"Asset": ["CB64D3G7SM2RTH6JSGG34DDTFTQ5CFDKVDZJZSODMCX4NJ2HV2KN7OHT", "100" ]}),
    )
    .await;
}

fn complex_tuple() -> serde_json::Value {
    json!({"Tuple": [{"a": 42, "b": true, "c": "world"}, "First"]})
}

#[tokio::test]
async fn e_2_s_tuple() {
    invoke_with_roundtrip("complex", complex_tuple()).await;
}

#[tokio::test]
async fn e_2_s_strukt() {
    invoke_with_roundtrip(
        "complex",
        json!({"Struct": {"a": 42, "b": true, "c": "world"}}),
    )
    .await;
}

#[tokio::test]
async fn number_arg() {
    invoke_with_roundtrip("u32_", 42).await;
}

#[test]
fn number_arg_return_ok() {
    invoke(&TestEnv::default(), "u32_fail_on_even")
        .arg("--u32_")
        .arg("1")
        .assert()
        .success()
        .stdout("1\n");
}

#[tokio::test]
async fn number_arg_return_err() {
    let sandbox = &TestEnv::default();

    let p = CUSTOM_TYPES.path();
    let wasm = p.to_str().unwrap();
    let res = sandbox
        .invoke(&[
            "--id=1",
            "--wasm",
            wasm,
            "--",
            "u32_fail_on_even",
            "--u32_=2",
        ])
        .await
        .unwrap_err();
    if let commands::contract::invoke::Error::ContractInvoke(name, doc) = &res {
        assert_eq!(name, "NumberMustBeOdd");
        assert_eq!(doc, "Please provide an odd number");
    };
    println!("{res:#?}");
}

#[test]
fn void() {
    invoke(&TestEnv::default(), "woid")
        .assert()
        .success()
        .stdout("\n")
        .stderr("");
}

#[test]
fn val() {
    invoke(&TestEnv::default(), "val")
        .assert()
        .success()
        .stdout("null\n")
        .stderr("");
}

#[tokio::test]
async fn i32() {
    invoke_with_roundtrip("i32_", 42).await;
}

#[test]
fn handle_arg_larger_than_i32_failure() {
    invoke(&TestEnv::default(), "i32_")
        .arg("--i32_")
        .arg(u32::MAX.to_string())
        .assert()
        .failure()
        .stderr(predicates::str::contains("value is not parseable"));
}

#[test]
fn handle_arg_larger_than_i64_failure() {
    invoke(&TestEnv::default(), "i64_")
        .arg("--i64_")
        .arg(u64::MAX.to_string())
        .assert()
        .failure()
        .stderr(predicates::str::contains("value is not parseable"));
}

#[tokio::test]
async fn i64() {
    invoke_with_roundtrip("i64_", i64::MAX).await;
}

#[tokio::test]
async fn negative_i32() {
    invoke_with_roundtrip("i32_", -42).await;
}

#[tokio::test]
async fn negative_i64() {
    invoke_with_roundtrip("i64_", i64::MIN).await;
}

#[tokio::test]
async fn account_address() {
    invoke_with_roundtrip(
        "addresse",
        json!("GD5KD2KEZJIGTC63IGW6UMUSMVUVG5IHG64HUTFWCHVZH2N2IBOQN7PS"),
    )
    .await;
}

#[tokio::test]
async fn contract_address() {
    invoke_with_roundtrip(
        "addresse",
        json!("CA3D5KRYM6CB7OWQ6TWYRR3Z4T7GNZLKERYNZGGA5SOAOPIFY6YQGAXE"),
    )
    .await;
}

#[tokio::test]
async fn bytes() {
    invoke_with_roundtrip("bytes", json!("7374656c6c6172")).await;
}

#[tokio::test]
async fn const_enum() {
    invoke_with_roundtrip("card", "11").await;
}

#[test]
fn parse_u128() {
    let num = "340000000000000000000000000000000000000";
    invoke(&TestEnv::default(), "u128")
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
    invoke(&TestEnv::default(), "i128")
        .arg("--i128")
        .arg(num)
        .assert()
        .success()
        .stdout(format!(
            r#""{num}"
"#,
        ));
}

#[test]
fn parse_negative_i128() {
    let num = "-170000000000000000000000000000000000000";
    invoke(&TestEnv::default(), "i128")
        .arg("--i128")
        .arg(num)
        .assert()
        .success()
        .stdout(format!(
            r#""{num}"
"#,
        ));
}

#[test]
fn parse_u256() {
    let num = "340000000000000000000000000000000000000";
    invoke(&TestEnv::default(), "u256")
        .arg("--u256")
        .arg(num)
        .assert()
        .success()
        .stdout(format!(
            r#""{num}"
"#,
        ));
}

#[test]
fn parse_i256() {
    let num = "170000000000000000000000000000000000000";
    invoke(&TestEnv::default(), "i256")
        .arg("--i256")
        .arg(num)
        .assert()
        .success()
        .stdout(format!(
            r#""{num}"
"#,
        ));
}

#[test]
fn parse_negative_i256() {
    let num = "-170000000000000000000000000000000000000";
    invoke(&TestEnv::default(), "i256")
        .arg("--i256")
        .arg(num)
        .assert()
        .success()
        .stdout(format!(
            r#""{num}"
"#,
        ));
}

#[test]
fn boolean() {
    invoke(&TestEnv::default(), "boolean")
        .arg("--boolean")
        .assert()
        .success()
        .stdout(
            r#"true
"#,
        );
}
#[test]
fn boolean_two() {
    invoke(&TestEnv::default(), "boolean")
        .arg("--boolean")
        .arg("true")
        .assert()
        .success()
        .stdout(
            r#"true
"#,
        );
}

#[test]
fn boolean_no_flag() {
    invoke(&TestEnv::default(), "boolean")
        .assert()
        .success()
        .stdout(
            r#"false
"#,
        );
}

#[test]
fn boolean_false() {
    invoke(&TestEnv::default(), "boolean")
        .arg("--boolean")
        .arg("false")
        .assert()
        .success()
        .stdout(
            r#"false
"#,
        );
}

#[test]
fn boolean_not() {
    invoke(&TestEnv::default(), "not")
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
    invoke(&TestEnv::default(), "not")
        .assert()
        .success()
        .stdout(
            r#"true
"#,
        );
}

#[test]
fn option_none() {
    invoke(&TestEnv::default(), "option")
        .assert()
        .success()
        .stdout(
            r#"null
"#,
        );
}

#[test]
fn option_some() {
    invoke(&TestEnv::default(), "option")
        .arg("--option=1")
        .assert()
        .success()
        .stdout(
            r#"1
"#,
        );
}
