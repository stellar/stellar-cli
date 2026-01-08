//! Filter unused types from contract spec entries.
//!
//! This module provides functionality to remove type definitions that are not
//! referenced by any function in the contract spec. This helps reduce WASM size
//! by eliminating unnecessary spec entries.

use std::collections::HashSet;

use stellar_xdr::curr::{
    ScSpecEntry, ScSpecTypeDef, ScSpecUdtStructV0, ScSpecUdtUnionCaseV0, ScSpecUdtUnionV0,
};

/// Extracts UDT (User Defined Type) names referenced by a type definition.
///
/// This function recursively traverses the type structure to find all
/// references to user-defined types.
fn get_type_refs(type_def: &ScSpecTypeDef) -> HashSet<String> {
    let mut refs = HashSet::new();

    match type_def {
        // Primitive types have no UDT references
        ScSpecTypeDef::Val
        | ScSpecTypeDef::U64
        | ScSpecTypeDef::I64
        | ScSpecTypeDef::U128
        | ScSpecTypeDef::I128
        | ScSpecTypeDef::U32
        | ScSpecTypeDef::I32
        | ScSpecTypeDef::U256
        | ScSpecTypeDef::I256
        | ScSpecTypeDef::Bool
        | ScSpecTypeDef::Symbol
        | ScSpecTypeDef::Error
        | ScSpecTypeDef::Bytes
        | ScSpecTypeDef::BytesN(_)
        | ScSpecTypeDef::Void
        | ScSpecTypeDef::Timepoint
        | ScSpecTypeDef::Duration
        | ScSpecTypeDef::String
        | ScSpecTypeDef::Address
        | ScSpecTypeDef::MuxedAddress => {}

        // UDT reference - add the type name
        ScSpecTypeDef::Udt(udt) => {
            refs.insert(udt.name.to_utf8_string_lossy());
        }

        // Composite types - recurse into contained types
        ScSpecTypeDef::Vec(vec_type) => {
            refs.extend(get_type_refs(&vec_type.element_type));
        }
        ScSpecTypeDef::Map(map_type) => {
            refs.extend(get_type_refs(&map_type.key_type));
            refs.extend(get_type_refs(&map_type.value_type));
        }
        ScSpecTypeDef::Option(opt_type) => {
            refs.extend(get_type_refs(&opt_type.value_type));
        }
        ScSpecTypeDef::Result(result_type) => {
            refs.extend(get_type_refs(&result_type.ok_type));
            refs.extend(get_type_refs(&result_type.error_type));
        }
        ScSpecTypeDef::Tuple(tuple_type) => {
            for value_type in tuple_type.value_types.iter() {
                refs.extend(get_type_refs(value_type));
            }
        }
    }

    refs
}

/// Extracts all UDT names referenced by a spec entry.
fn get_entry_type_refs(entry: &ScSpecEntry) -> HashSet<String> {
    let mut refs = HashSet::new();

    match entry {
        ScSpecEntry::FunctionV0(func) => {
            // Collect types from inputs
            for input in func.inputs.iter() {
                refs.extend(get_type_refs(&input.type_));
            }
            // Collect types from outputs
            for output in func.outputs.iter() {
                refs.extend(get_type_refs(output));
            }
        }
        ScSpecEntry::UdtStructV0(ScSpecUdtStructV0 { fields, .. }) => {
            for field in fields.iter() {
                refs.extend(get_type_refs(&field.type_));
            }
        }
        ScSpecEntry::UdtUnionV0(ScSpecUdtUnionV0 { cases, .. }) => {
            for case in cases.iter() {
                if let ScSpecUdtUnionCaseV0::TupleV0(tuple_case) = case {
                    for type_def in tuple_case.type_.iter() {
                        refs.extend(get_type_refs(type_def));
                    }
                }
            }
        }
        // Enums, error enums, and events don't reference other types
        ScSpecEntry::UdtEnumV0(_) | ScSpecEntry::UdtErrorEnumV0(_) | ScSpecEntry::EventV0(_) => {}
    }

    refs
}

/// Gets the name of a UDT entry, or None if it's not a UDT.
fn get_udt_name(entry: &ScSpecEntry) -> Option<String> {
    match entry {
        ScSpecEntry::UdtStructV0(s) => Some(s.name.to_utf8_string_lossy()),
        ScSpecEntry::UdtUnionV0(u) => Some(u.name.to_utf8_string_lossy()),
        ScSpecEntry::UdtEnumV0(e) => Some(e.name.to_utf8_string_lossy()),
        ScSpecEntry::UdtErrorEnumV0(e) => Some(e.name.to_utf8_string_lossy()),
        ScSpecEntry::FunctionV0(_) | ScSpecEntry::EventV0(_) => None,
    }
}

/// Filters out unused types from contract spec entries.
///
/// This function performs a reachability analysis starting from all functions.
/// It keeps:
/// - All functions (FunctionV0)
/// - All events (EventV0)
/// - All UDTs that are directly or transitively referenced by functions
///
/// Types that are defined but never used by any function are removed.
///
/// # Example
///
/// If a contract has:
/// - Function `foo` that takes `TypeA` as input
/// - `TypeA` which references `TypeB` in a field
/// - `TypeC` which is defined but never used
///
/// The result will include `foo`, `TypeA`, and `TypeB`, but not `TypeC`.
pub fn filter_unused_types(entries: Vec<ScSpecEntry>) -> Vec<ScSpecEntry> {
    // Build a map from type name to entry for lookup
    let type_entries: std::collections::HashMap<String, &ScSpecEntry> = entries
        .iter()
        .filter_map(|entry| get_udt_name(entry).map(|name| (name, entry)))
        .collect();

    // Collect initial references from all functions
    let mut reachable_types: HashSet<String> = HashSet::new();
    for entry in &entries {
        if matches!(entry, ScSpecEntry::FunctionV0(_)) {
            reachable_types.extend(get_entry_type_refs(entry));
        }
    }

    // Fixed-point iteration: keep adding types referenced by reachable types
    // until no new types are found
    loop {
        let mut new_types: HashSet<String> = HashSet::new();

        for type_name in &reachable_types {
            if let Some(entry) = type_entries.get(type_name) {
                for referenced_type in get_entry_type_refs(entry) {
                    if !reachable_types.contains(&referenced_type) {
                        new_types.insert(referenced_type);
                    }
                }
            }
        }

        if new_types.is_empty() {
            break;
        }

        reachable_types.extend(new_types);
    }

    // Filter entries: keep functions, events, and reachable UDTs
    entries
        .into_iter()
        .filter(|entry| {
            match entry {
                // Always keep functions and events
                ScSpecEntry::FunctionV0(_) | ScSpecEntry::EventV0(_) => true,
                // Keep UDTs only if they're reachable
                _ => {
                    if let Some(name) = get_udt_name(entry) {
                        reachable_types.contains(&name)
                    } else {
                        true
                    }
                }
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use stellar_xdr::curr::{
        ScSpecFunctionInputV0, ScSpecFunctionV0, ScSpecTypeUdt, ScSpecUdtEnumCaseV0,
        ScSpecUdtEnumV0, ScSpecUdtErrorEnumCaseV0, ScSpecUdtErrorEnumV0, ScSpecUdtStructFieldV0,
        StringM, VecM,
    };

    fn make_function(name: &str, input_types: Vec<ScSpecTypeDef>) -> ScSpecEntry {
        let inputs: VecM<ScSpecFunctionInputV0, 10> = input_types
            .into_iter()
            .enumerate()
            .map(|(i, type_)| ScSpecFunctionInputV0 {
                doc: StringM::default(),
                name: format!("arg{i}").try_into().unwrap(),
                type_,
            })
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();

        ScSpecEntry::FunctionV0(ScSpecFunctionV0 {
            doc: StringM::default(),
            name: name.try_into().unwrap(),
            inputs,
            outputs: VecM::default(),
        })
    }

    fn make_struct(name: &str, field_types: Vec<(&str, ScSpecTypeDef)>) -> ScSpecEntry {
        let fields: VecM<ScSpecUdtStructFieldV0, 40> = field_types
            .into_iter()
            .map(|(field_name, type_)| ScSpecUdtStructFieldV0 {
                doc: StringM::default(),
                name: field_name.try_into().unwrap(),
                type_,
            })
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();

        ScSpecEntry::UdtStructV0(ScSpecUdtStructV0 {
            doc: StringM::default(),
            lib: StringM::default(),
            name: name.try_into().unwrap(),
            fields,
        })
    }

    fn make_enum(name: &str) -> ScSpecEntry {
        ScSpecEntry::UdtEnumV0(ScSpecUdtEnumV0 {
            doc: StringM::default(),
            lib: StringM::default(),
            name: name.try_into().unwrap(),
            cases: vec![ScSpecUdtEnumCaseV0 {
                doc: StringM::default(),
                name: "Variant".try_into().unwrap(),
                value: 0,
            }]
            .try_into()
            .unwrap(),
        })
    }

    fn make_error_enum(name: &str) -> ScSpecEntry {
        ScSpecEntry::UdtErrorEnumV0(ScSpecUdtErrorEnumV0 {
            doc: StringM::default(),
            lib: StringM::default(),
            name: name.try_into().unwrap(),
            cases: vec![ScSpecUdtErrorEnumCaseV0 {
                doc: StringM::default(),
                name: "Error".try_into().unwrap(),
                value: 1,
            }]
            .try_into()
            .unwrap(),
        })
    }

    fn udt(name: &str) -> ScSpecTypeDef {
        ScSpecTypeDef::Udt(ScSpecTypeUdt {
            name: name.try_into().unwrap(),
        })
    }

    #[test]
    fn test_removes_unused_type() {
        let entries = vec![
            make_function("foo", vec![ScSpecTypeDef::U32]),
            make_struct("UsedType", vec![("field", ScSpecTypeDef::U32)]),
            make_struct("UnusedType", vec![("field", ScSpecTypeDef::U32)]),
        ];

        let filtered = filter_unused_types(entries);

        assert_eq!(filtered.len(), 1);
        assert!(matches!(filtered[0], ScSpecEntry::FunctionV0(_)));
    }

    #[test]
    fn test_keeps_directly_referenced_type() {
        let entries = vec![
            make_function("foo", vec![udt("UsedType")]),
            make_struct("UsedType", vec![("field", ScSpecTypeDef::U32)]),
            make_struct("UnusedType", vec![("field", ScSpecTypeDef::U32)]),
        ];

        let filtered = filter_unused_types(entries);

        assert_eq!(filtered.len(), 2);

        let names: Vec<_> = filtered.iter().filter_map(get_udt_name).collect();
        assert!(names.contains(&"UsedType".to_string()));
        assert!(!names.contains(&"UnusedType".to_string()));
    }

    #[test]
    fn test_keeps_transitively_referenced_type() {
        let entries = vec![
            make_function("foo", vec![udt("TypeA")]),
            make_struct("TypeA", vec![("field", udt("TypeB"))]),
            make_struct("TypeB", vec![("field", ScSpecTypeDef::U32)]),
            make_struct("UnusedType", vec![("field", ScSpecTypeDef::U32)]),
        ];

        let filtered = filter_unused_types(entries);

        let names: Vec<_> = filtered.iter().filter_map(get_udt_name).collect();
        assert!(names.contains(&"TypeA".to_string()));
        assert!(names.contains(&"TypeB".to_string()));
        assert!(!names.contains(&"UnusedType".to_string()));
    }

    #[test]
    fn test_keeps_all_functions() {
        let entries = vec![
            make_function("foo", vec![ScSpecTypeDef::U32]),
            make_function("bar", vec![ScSpecTypeDef::Bool]),
        ];

        let filtered = filter_unused_types(entries);

        assert_eq!(filtered.len(), 2);
        assert!(filtered
            .iter()
            .all(|e| matches!(e, ScSpecEntry::FunctionV0(_))));
    }

    #[test]
    fn test_removes_unused_error_enum() {
        let entries = vec![
            make_function("foo", vec![ScSpecTypeDef::U32]),
            make_error_enum("UsedError"),
            make_error_enum("UnusedError"),
        ];

        let filtered = filter_unused_types(entries);

        // Only function should remain, no error enums are referenced
        assert_eq!(filtered.len(), 1);
        assert!(matches!(filtered[0], ScSpecEntry::FunctionV0(_)));
    }

    #[test]
    fn test_keeps_error_enum_in_result() {
        let entries = vec![
            make_function(
                "foo",
                vec![ScSpecTypeDef::Result(Box::new(
                    stellar_xdr::curr::ScSpecTypeResult {
                        ok_type: Box::new(ScSpecTypeDef::U32),
                        error_type: Box::new(udt("MyError")),
                    },
                ))],
            ),
            make_error_enum("MyError"),
            make_error_enum("UnusedError"),
        ];

        let filtered = filter_unused_types(entries);

        let names: Vec<_> = filtered.iter().filter_map(get_udt_name).collect();
        assert!(names.contains(&"MyError".to_string()));
        assert!(!names.contains(&"UnusedError".to_string()));
    }

    #[test]
    fn test_handles_circular_references() {
        // TypeA references TypeB, TypeB references TypeA
        let entries = vec![
            make_function("foo", vec![udt("TypeA")]),
            make_struct("TypeA", vec![("b", udt("TypeB"))]),
            make_struct("TypeB", vec![("a", udt("TypeA"))]),
        ];

        let filtered = filter_unused_types(entries);

        let names: Vec<_> = filtered.iter().filter_map(get_udt_name).collect();
        assert!(names.contains(&"TypeA".to_string()));
        assert!(names.contains(&"TypeB".to_string()));
    }

    #[test]
    fn test_handles_vec_of_udt() {
        let entries = vec![
            make_function(
                "foo",
                vec![ScSpecTypeDef::Vec(Box::new(
                    stellar_xdr::curr::ScSpecTypeVec {
                        element_type: Box::new(udt("MyType")),
                    },
                ))],
            ),
            make_struct("MyType", vec![("field", ScSpecTypeDef::U32)]),
        ];

        let filtered = filter_unused_types(entries);

        let names: Vec<_> = filtered.iter().filter_map(get_udt_name).collect();
        assert!(names.contains(&"MyType".to_string()));
    }

    #[test]
    fn test_handles_map_with_udt() {
        let entries = vec![
            make_function(
                "foo",
                vec![ScSpecTypeDef::Map(Box::new(
                    stellar_xdr::curr::ScSpecTypeMap {
                        key_type: Box::new(udt("KeyType")),
                        value_type: Box::new(udt("ValueType")),
                    },
                ))],
            ),
            make_struct("KeyType", vec![("field", ScSpecTypeDef::U32)]),
            make_struct("ValueType", vec![("field", ScSpecTypeDef::U32)]),
            make_struct("UnusedType", vec![("field", ScSpecTypeDef::U32)]),
        ];

        let filtered = filter_unused_types(entries);

        let names: Vec<_> = filtered.iter().filter_map(get_udt_name).collect();
        assert!(names.contains(&"KeyType".to_string()));
        assert!(names.contains(&"ValueType".to_string()));
        assert!(!names.contains(&"UnusedType".to_string()));
    }

    #[test]
    fn test_keeps_enum_referenced_by_function() {
        let entries = vec![
            make_function("foo", vec![udt("MyEnum")]),
            make_enum("MyEnum"),
            make_enum("UnusedEnum"),
        ];

        let filtered = filter_unused_types(entries);

        let names: Vec<_> = filtered.iter().filter_map(get_udt_name).collect();
        assert!(names.contains(&"MyEnum".to_string()));
        assert!(!names.contains(&"UnusedEnum".to_string()));
    }
}
