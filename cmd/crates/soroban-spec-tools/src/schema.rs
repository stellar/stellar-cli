use serde_json::{json, Value};
use stellar_xdr::curr::{self as xdr, ScSpecTypeDef as ScType};

use crate::{Error, Spec};

impl Spec {
    pub fn to_json_schema(&self) -> Result<Value, Error> {
        let mut definitions = serde_json::Map::new();
        let mut properties = serde_json::Map::new();

        if let Some(entries) = &self.0 {
            for entry in entries {
                match entry {
                    xdr::ScSpecEntry::FunctionV0(function) => {
                        let function_schema = self.function_to_json_schema(function)?;
                        properties.insert(function.name.to_utf8_string_lossy(), function_schema);
                    }
                    xdr::ScSpecEntry::UdtStructV0(struct_) => {
                        let struct_schema = self.struct_to_json_schema(struct_)?;
                        definitions.insert(struct_.name.to_utf8_string_lossy(), struct_schema);
                    }
                    xdr::ScSpecEntry::UdtUnionV0(union) => {
                        let union_schema = self.union_to_json_schema(union)?;
                        definitions.insert(union.name.to_utf8_string_lossy(), union_schema);
                    }
                    xdr::ScSpecEntry::UdtEnumV0(enum_) => {
                        let enum_schema = self.enum_to_json_schema(enum_)?;
                        definitions.insert(enum_.name.to_utf8_string_lossy(), enum_schema);
                    }
                    xdr::ScSpecEntry::UdtErrorEnumV0(error_enum) => {
                        let error_enum_schema = self.error_enum_to_json_schema(error_enum)?;
                        definitions
                            .insert(error_enum.name.to_utf8_string_lossy(), error_enum_schema);
                    }
                }
            }
        }

        Ok(json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "type": "object",
            "properties": properties,
            "definitions": definitions
        }))
    }

    fn function_to_json_schema(&self, function: &xdr::ScSpecFunctionV0) -> Result<Value, Error> {
        let mut properties = serde_json::Map::new();
        for param in function.inputs.iter() {
            let param_schema = self.type_to_json_schema(&param.type_)?;
            properties.insert(param.name.to_utf8_string_lossy(), param_schema);
        }

        Ok(json!({
            "type": "object",
            "properties": properties,
            "required": function.inputs.iter().map(|p| p.name.to_utf8_string_lossy()).collect::<Vec<_>>()
        }))
    }

    fn struct_to_json_schema(&self, struct_: &xdr::ScSpecUdtStructV0) -> Result<Value, Error> {
        let mut properties = serde_json::Map::new();
        for field in struct_.fields.iter() {
            let field_schema = self.type_to_json_schema(&field.type_)?;
            properties.insert(field.name.to_utf8_string_lossy(), field_schema);
        }

        Ok(json!({
            "type": "object",
            "properties": properties,
            "required": struct_.fields.iter().map(|f| f.name.to_utf8_string_lossy()).collect::<Vec<_>>()
        }))
    }

    fn union_to_json_schema(&self, union: &xdr::ScSpecUdtUnionV0) -> Result<Value, Error> {
        let mut one_of = Vec::new();
        for case in union.cases.iter() {
            match case {
                xdr::ScSpecUdtUnionCaseV0::VoidV0(void_case) => {
                    one_of.push(json!({
                        "type": "string",
                        "enum": [void_case.name.to_utf8_string_lossy()]
                    }));
                }
                xdr::ScSpecUdtUnionCaseV0::TupleV0(tuple_case) => {
                    let mut properties = serde_json::Map::new();
                    properties.insert(tuple_case.name.to_utf8_string_lossy(), json!({
                        "type": "array",
                        "items": tuple_case.type_.iter().map(|t| self.type_to_json_schema(t).unwrap()).collect::<Vec<_>>()
                    }));
                    one_of.push(json!({
                        "type": "object",
                        "properties": properties,
                        "required": [tuple_case.name.to_utf8_string_lossy()]
                    }));
                }
            }
        }

        Ok(json!({ "oneOf": one_of }))
    }

    fn enum_to_json_schema(&self, enum_: &xdr::ScSpecUdtEnumV0) -> Result<Value, Error> {
        Ok(json!({
            "type": "integer",
            "enum": enum_.cases.iter().map(|c| c.value).collect::<Vec<_>>()
        }))
    }

    fn error_enum_to_json_schema(
        &self,
        error_enum: &xdr::ScSpecUdtErrorEnumV0,
    ) -> Result<Value, Error> {
        Ok(json!({
            "type": "integer",
            "enum": error_enum.cases.iter().map(|c| c.value).collect::<Vec<_>>()
        }))
    }

    fn type_to_json_schema(&self, type_: &ScType) -> Result<Value, Error> {
        Ok(match type_ {
            ScType::Bool => json!({"type": "boolean"}),
            ScType::Void => json!({"type": "null"}),
            ScType::Error => {
                json!({"type": "object", "properties": {"Error": {"type": "integer"}}})
            }
            ScType::U32 | ScType::I32 | ScType::U64 | ScType::I64 => {
                json!({"type": "integer"})
            }
            ScType::U128 | ScType::I128 | ScType::U256 | ScType::I256 => {
                json!({"type": "string"})
            }
            ScType::Bytes | ScType::String | ScType::Symbol => {
                json!({"type": "string"})
            }
            ScType::Vec(vec_type) => json!({
                "type": "array",
                "items": self.type_to_json_schema(&vec_type.element_type)?
            }),
            ScType::Map(map_type) => json!({
                "type": "object",
                "additionalProperties": self.type_to_json_schema(&map_type.value_type)?
            }),
            ScType::Option(option_type) => json!({
                "oneOf": [
                    {"type": "null"},
                    self.type_to_json_schema(&option_type.value_type)?
                ]
            }),
            ScType::Result(result_type) => json!({
                "oneOf": [
                    {"type": "object", "properties": {"Ok": self.type_to_json_schema(&result_type.ok_type)?}},
                    {"type": "object", "properties": {"Error": self.type_to_json_schema(&result_type.error_type)?}}
                ]
            }),
            ScType::Tuple(tuple_type) => json!({
                "type": "array",
                "items": tuple_type.value_types.iter().map(|t| self.type_to_json_schema(t).unwrap()).collect::<Vec<_>>(),
                "minItems": tuple_type.value_types.len(),
                "maxItems": tuple_type.value_types.len()
            }),
            ScType::BytesN(bytes_n) => json!({
                "type": "string",
                "pattern": format!("^[0-9a-fA-F]{{{}}}$", bytes_n.n * 2)
            }),
            ScType::Address => json!({"type": "string", "pattern": "^[GC][A-Z2-7]{55}$"}),
            ScType::Timepoint | ScType::Duration => json!({"type": "integer"}),
            ScType::Udt(udt_type) => {
                json!({"$ref": format!("#/definitions/{}", udt_type.name.to_utf8_string_lossy())})
            }
            ScType::Val => json!({}), // Allow any type for Val
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate() {
        let wasm_bytes = include_bytes!(
            "../../../../target/wasm32-unknown-unknown/test-wasms/test_hello_world.wasm"
        );
        let spec = Spec::from_wasm(wasm_bytes).unwrap();
        let json_schema = spec.to_json_schema().unwrap();
        println!("{}", serde_json::to_string_pretty(&json_schema).unwrap());
    }
}
