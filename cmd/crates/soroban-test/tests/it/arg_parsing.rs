use crate::util::CUSTOM_TYPES;
use serde_json::json;
use soroban_cli::xdr::{
    Duration, Int128Parts, Int256Parts, ScBytes, ScSpecTypeBytesN, ScSpecTypeDef, ScSpecTypeOption,
    ScSpecTypeUdt, ScVal, TimePoint, UInt128Parts, UInt256Parts,
};
use soroban_spec_tools::{from_string_primitive, Spec};

#[test]
fn parse_bool() {
    let parsed = from_string_primitive("true", &ScSpecTypeDef::Bool).unwrap();
    assert!(parsed == ScVal::Bool(true));
}

#[test]
fn parse_null() {
    let parsed = from_string_primitive(
        "null",
        &ScSpecTypeDef::Option(Box::new(ScSpecTypeOption {
            value_type: Box::new(ScSpecTypeDef::Bool),
        })),
    )
    .unwrap();
    assert!(parsed == ScVal::Void);
}

#[test]
fn parse_u32() {
    let u32_ = 42u32;
    let res = &format!("{u32_}");
    let parsed = from_string_primitive(res, &ScSpecTypeDef::U32).unwrap();
    assert!(parsed == ScVal::U32(u32_));
}

#[test]
fn parse_i32() {
    let i32_ = -42_i32;
    let res = &format!("{i32_}");
    let parsed = from_string_primitive(res, &ScSpecTypeDef::I32).unwrap();
    assert!(parsed == ScVal::I32(i32_));
}

#[test]
fn parse_u64() {
    let b = 42_000_000_000u64;
    let res = &format!("{b}");
    let parsed = from_string_primitive(res, &ScSpecTypeDef::U64).unwrap();
    assert!(parsed == ScVal::U64(b));
}

#[test]
#[allow(clippy::cast_possible_truncation)]
fn parse_u128() {
    let b = 340_000_000_000_000_000_000_000_000_000_000_000_000u128;
    let res = &format!("{b}");
    let lo = b as u64;
    let hi = (b >> 64) as u64;
    let parsed = from_string_primitive(res, &ScSpecTypeDef::U128).unwrap();
    assert!(parsed == ScVal::U128(UInt128Parts { hi, lo }));
}

#[test]
#[allow(clippy::cast_possible_truncation)]
fn parse_u256() {
    let b = 340_000_000_000_000_000_000_000_000_000_000_000_000u128;
    let res = &format!("{b}");
    let lo_lo = b as u64;
    let lo_hi = (b >> 64) as u64;
    let hi_lo = 0u64;
    let hi_hi = 0u64;
    let parsed = from_string_primitive(res, &ScSpecTypeDef::U256).unwrap();
    assert!(
        parsed
            == ScVal::U256(UInt256Parts {
                lo_lo,
                lo_hi,
                hi_lo,
                hi_hi
            })
    );
}

#[test]
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn parse_i128() {
    let b = -170_000_000_000_000_000_000_000_000_000_000_000_000i128;
    let res = &format!("{b}");
    let lo = b as u64;
    let hi = (b >> 64) as i64;
    let parsed = from_string_primitive(res, &ScSpecTypeDef::I128).unwrap();
    assert!(parsed == ScVal::I128(Int128Parts { hi, lo }));
}

#[test]
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn parse_i256() {
    let b = -170_000_000_000_000_000_000_000_000_000_000_000_000i128;
    let res = &format!("{b}");
    let lo_lo = b as u64;
    let lo_hi = (b >> 64) as u64;
    // b is negative i128, so hi parts are all ones
    let hi_lo = u64::MAX;
    let hi_hi = -1i64;
    let parsed = from_string_primitive(res, &ScSpecTypeDef::I256).unwrap();
    assert!(
        parsed
            == ScVal::I256(Int256Parts {
                lo_lo,
                lo_hi,
                hi_lo,
                hi_hi
            })
    );
}

#[test]
fn parse_bytes() {
    let b = from_string_primitive(r"beefface", &ScSpecTypeDef::Bytes).unwrap();
    assert_eq!(
        b,
        ScVal::Bytes(ScBytes(vec![0xbe, 0xef, 0xfa, 0xce].try_into().unwrap()))
    );
}

#[test]
fn parse_bytes_when_hex_is_all_numbers() {
    let b = from_string_primitive(r"4554", &ScSpecTypeDef::Bytes).unwrap();
    assert_eq!(
        b,
        ScVal::Bytes(ScBytes(vec![0x45, 0x54].try_into().unwrap()))
    );
}

#[test]
fn parse_bytesn() {
    let b = from_string_primitive(
        r"beefface",
        &ScSpecTypeDef::BytesN(ScSpecTypeBytesN { n: 4 }),
    )
    .unwrap();
    assert_eq!(
        b,
        ScVal::Bytes(ScBytes(vec![0xbe, 0xef, 0xfa, 0xce].try_into().unwrap()))
    );
}

#[test]
fn parse_bytesn_when_hex_is_all_numbers() {
    let b =
        from_string_primitive(r"4554", &ScSpecTypeDef::BytesN(ScSpecTypeBytesN { n: 2 })).unwrap();
    assert_eq!(
        b,
        ScVal::Bytes(ScBytes(vec![0x45, 0x54].try_into().unwrap()))
    );
}

#[test]
fn parse_timepoint() {
    let b = 1_760_501_234u64;
    let res = &format!("{b}");
    let parsed = from_string_primitive(res, &ScSpecTypeDef::Timepoint).unwrap();
    assert!(parsed == ScVal::Timepoint(TimePoint::from(b)));
}

#[test]
fn parse_duration() {
    let b = 1_234_567u64;
    let res = &format!("{b}");
    let parsed = from_string_primitive(res, &ScSpecTypeDef::Duration).unwrap();
    assert!(parsed == ScVal::Duration(Duration::from(b)));
}

#[test]
fn parse_symbol() {
    let parsed = from_string_primitive(r#""hello""#, &ScSpecTypeDef::Symbol).unwrap();
    assert!(parsed == ScVal::Symbol("hello".try_into().unwrap()));
}

#[test]
fn parse_symbol_with_no_quotation_marks() {
    let parsed = from_string_primitive("hello", &ScSpecTypeDef::Symbol).unwrap();
    assert!(parsed == ScVal::Symbol("hello".try_into().unwrap()));
}

#[test]
fn parse_optional_symbol_with_no_quotation_marks() {
    let parsed = from_string_primitive(
        "hello",
        &ScSpecTypeDef::Option(Box::new(ScSpecTypeOption {
            value_type: Box::new(ScSpecTypeDef::Symbol),
        })),
    )
    .unwrap();
    assert!(parsed == ScVal::Symbol("hello".try_into().unwrap()));
}

#[test]
fn parse_optional_bool_with_no_quotation_marks() {
    let parsed = from_string_primitive(
        "true",
        &ScSpecTypeDef::Option(Box::new(ScSpecTypeOption {
            value_type: Box::new(ScSpecTypeDef::Bool),
        })),
    )
    .unwrap();
    assert!(parsed == ScVal::Bool(true));
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
