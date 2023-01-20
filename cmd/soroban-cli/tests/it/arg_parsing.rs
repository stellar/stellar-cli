use crate::util::CUSTOM_TYPES;
use serde_json::json;
use soroban_cli::strval::{self, Spec};
use soroban_env_host::xdr::{ScSpecTypeDef, ScSpecTypeOption, ScSpecTypeUdt, ScStatic, ScVal};

#[test]
fn parse_bool() {
    println!(
        "{:#?}",
        strval::from_string_primitive("true", &ScSpecTypeDef::Bool,).unwrap()
    );
}

#[test]
fn parse_null() {
    let parsed = strval::from_string_primitive(
        "null",
        &ScSpecTypeDef::Option(Box::new(ScSpecTypeOption {
            value_type: Box::new(ScSpecTypeDef::Bool),
        })),
    )
    .unwrap();
    println!("{:#?}", parsed);
    assert!(parsed == ScVal::Static(ScStatic::Void))
}

#[test]
fn parse_u32() {
    let u32_ = 42u32;
    let res = &format!("{u32_}");
    println!(
        "{:#?}",
        strval::from_string_primitive(res, &ScSpecTypeDef::U32,).unwrap()
    );
}

#[test]
fn parse_i32() {
    let i32_ = -42_i32;
    let res = &format!("{i32_}");
    println!(
        "{:#?}",
        strval::from_string_primitive(res, &ScSpecTypeDef::I32,).unwrap()
    );
}

#[test]
fn parse_u64() {
    let b = 42_000_000_000u64;
    let res = &format!("{b}");
    println!(
        "{:#?}",
        strval::from_string_primitive(res, &ScSpecTypeDef::U64,).unwrap()
    );
}

#[test]
fn parse_symbol() {
    // let b = "hello";
    // let res = &parse_json(&HashMap::new(), &ScSpecTypeDef::Symbol, &json! {b}).unwrap();
    // println!("{res}");
    println!(
        "{:#?}",
        strval::from_string_primitive(r#""hello""#, &ScSpecTypeDef::Symbol).unwrap()
    );
}

#[test]
fn parse_symbol_with_no_quotation_marks() {
    // let b = "hello";
    // let res = &parse_json(&HashMap::new(), &ScSpecTypeDef::Symbol, &json! {b}).unwrap();
    // println!("{res}");
    println!(
        "{:#?}",
        strval::from_string_primitive("hello", &ScSpecTypeDef::Symbol).unwrap()
    );
}

#[test]
fn parse_obj() {
    let type_ = &ScSpecTypeDef::Udt(ScSpecTypeUdt {
        name: "Test".parse().unwrap(),
    });
    let entries = get_spec();
    let val = &json!({"a": 42, "b": false, "c": "world"});
    println!("{:#?}", entries.from_json(val, type_));
}

#[test]
fn parse_enum() {
    let entries = get_spec();
    let func = entries.find_function("simple").unwrap();
    println!("{func:#?}");
    let type_ = &func.inputs.as_slice()[0].type_;
    println!("{:#?}", entries.from_json(&json!("First"), type_));
}

#[test]
fn parse_enum_const() {
    let entries = get_spec();
    let func = entries.find_function("card").unwrap();
    println!("{func:#?}");
    let type_ = &func.inputs.as_slice()[0].type_;
    println!("{:#?}", entries.from_json(&json!(11), type_));
}

fn get_spec() -> Spec {
    let res = soroban_spec::read::from_wasm(&CUSTOM_TYPES.bytes()).unwrap();
    Spec(Some(res))
}
