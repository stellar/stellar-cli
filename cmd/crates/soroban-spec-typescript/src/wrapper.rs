use itertools::Itertools;

use crate::types;

pub fn type_to_js_xdr(value: &types::Type) -> String {
    match value {
        types::Type::Val => todo!(),
        types::Type::U64 => "xdr.ScVal.scvU64(xdr.Uint64.fromString(i.toString()))".to_string(),
        types::Type::I64 => "xdr.ScVal.scvI64(xdr.Int64.fromString(i.toString()))".to_string(),
        types::Type::U32 => "xdr.ScVal.scvU32(i)".to_string(),
        types::Type::I32 => "xdr.ScVal.scvI32(i)".to_string(),
        types::Type::Bool => "xdr.ScVal.scvBool(i)".to_string(),
        types::Type::Symbol => "xdr.ScVal.scvSymbol(i)".to_string(),
        types::Type::Map { key, value } => format!(
            "xdr.ScVal.scvMap(Array.from(i.entries()).map(([key, value]) => {{
            return new xdr.ScMapEntry({{
                key: ((i)=>{})(key),
                val: ((i)=>{})(value)}})
          }}))",
            type_to_js_xdr(key),
            type_to_js_xdr(value)
        ),
        types::Type::Option { value } => format!(
            "(!i) ? {} : {}",
            type_to_js_xdr(&types::Type::Void),
            type_to_js_xdr(value)
        ),
        types::Type::Result { value, .. } => type_to_js_xdr(value),
        types::Type::Vec { element } => {
            format!("xdr.ScVal.scvVec(i.map((i)=>{}))", type_to_js_xdr(element))
        }
        types::Type::Tuple { elements } => {
            let cases = elements
                .iter()
                .enumerate()
                .map(|(i, e)| format!("((i) => {})(i[{i}])", type_to_js_xdr(e)))
                .join(",\n        ");
            format!("xdr.ScVal.scvVec([{cases}])")
        }

        types::Type::Custom { name } => format!("{name}ToXdr(i)"),
        types::Type::BytesN { .. } | types::Type::Bytes => "xdr.ScVal.scvBytes(i)".to_owned(),
        types::Type::Address => "addressToScVal(i)".to_owned(),
        types::Type::Void => "xdr.ScVal.scvVoid()".to_owned(),
        types::Type::U128 => "u128ToScVal(i)".to_owned(),
        types::Type::I128 => "i128ToScVal(i)".to_owned(),

        types::Type::Set { .. }
        | types::Type::U256
        | types::Type::I256
        | types::Type::Timepoint
        | types::Type::Duration => "i".to_owned(),
        // This is case shoudn't happen since we only go xdr -> js for errors
        types::Type::Error { .. } => "N/A".to_owned(),
        types::Type::String => "xdr.ScVal.scvString(i)".to_owned(),
    }
}
