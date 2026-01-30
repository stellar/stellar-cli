//! Filter unused types from contract spec entries.
//!
//! This module provides functionality to remove type definitions that are not
//! referenced by any function in the contract spec. This helps reduce WASM size
//! by eliminating unnecessary spec entries.

use std::collections::HashSet;

use sha2::{Digest, Sha256};
use stellar_xdr::curr::{
    Limits, ScSpecEntry, ScSpecTypeDef, ScSpecUdtStructV0, ScSpecUdtUnionCaseV0, ScSpecUdtUnionV0,
    WriteXdr,
};

/// Magic bytes that identify a spec marker: `SpEc`
pub const SPEC_MARKER_MAGIC: [u8; 4] = [b'S', b'p', b'E', b'c'];

/// Length of the hash portion (truncated SHA256 - first 8 bytes / 64 bits).
pub const SPEC_MARKER_HASH_LEN: usize = 8;

/// Length of the marker: 4-byte prefix + 8-byte truncated SHA256 hash.
pub const SPEC_MARKER_LEN: usize = 4 + SPEC_MARKER_HASH_LEN;

/// A spec marker hash found in the WASM data section.
/// This is an 8-byte truncated SHA256 hash of the spec entry XDR bytes.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SpecMarkerHash(pub [u8; SPEC_MARKER_HASH_LEN]);

/// Computes the marker hash for a spec entry.
///
/// The hash is a truncated SHA256 (first 8 bytes) of the spec entry's XDR bytes.
///
/// # Panics
///
/// Panics if the spec entry cannot be encoded to XDR, which should never happen
/// for valid `ScSpecEntry` values.
pub fn compute_marker_hash(entry: &ScSpecEntry) -> SpecMarkerHash {
    let xdr_bytes = entry
        .to_xdr(Limits::none())
        .expect("XDR encoding should not fail");
    let mut hasher = Sha256::new();
    hasher.update(&xdr_bytes);
    let hash: [u8; 32] = hasher.finalize().into();
    let mut truncated = [0u8; SPEC_MARKER_HASH_LEN];
    truncated.copy_from_slice(&hash[..SPEC_MARKER_HASH_LEN]);
    SpecMarkerHash(truncated)
}

/// Extracts spec markers from the WASM data section.
///
/// The SDK embeds markers in the data section for each spec entry that is
/// actually used in the contract. These markers survive dead code elimination
/// only if the corresponding type/event is used.
///
/// Marker format:
/// - 4 bytes: `SpEc` magic
/// - 8 bytes: truncated SHA256 hash of the spec entry XDR bytes
pub fn extract_spec_markers(wasm_bytes: &[u8]) -> HashSet<SpecMarkerHash> {
    let mut markers = HashSet::new();

    for payload in wasmparser::Parser::new(0).parse_all(wasm_bytes) {
        let Ok(payload) = payload else { continue };

        if let wasmparser::Payload::DataSection(reader) = payload {
            for data in reader.into_iter().flatten() {
                extract_markers_from_data(data.data, &mut markers);
            }
        }
    }

    markers
}

/// Extracts spec markers from a data segment.
fn extract_markers_from_data(data: &[u8], markers: &mut HashSet<SpecMarkerHash>) {
    // Marker size is exactly 12 bytes: 4 (magic) + 8 (hash)
    if data.len() < SPEC_MARKER_LEN {
        return;
    }

    for i in 0..=data.len() - SPEC_MARKER_LEN {
        // Look for magic bytes
        if data[i..].starts_with(&SPEC_MARKER_MAGIC) {
            let hash_start = i + 4;
            let hash_end = hash_start + SPEC_MARKER_HASH_LEN;
            let mut hash = [0u8; SPEC_MARKER_HASH_LEN];
            hash.copy_from_slice(&data[hash_start..hash_end]);
            markers.insert(SpecMarkerHash(hash));
        }
    }
}

/// Filters spec entries based on markers found in the WASM data section.
///
/// This removes any spec entries (types, events) that don't have corresponding
/// markers in the data section. The SDK embeds markers for types/events that
/// are actually used, and these markers survive dead code elimination.
///
/// Functions are always kept as they define the contract's API.
///
/// # Arguments
///
/// * `entries` - The spec entries to filter
/// * `markers` - Marker hashes extracted from the WASM data section
///
/// # Returns
///
/// Filtered entries with only used types/events remaining.
#[allow(clippy::implicit_hasher)]
pub fn filter_by_markers(
    entries: Vec<ScSpecEntry>,
    markers: &HashSet<SpecMarkerHash>,
) -> Vec<ScSpecEntry> {
    entries
        .into_iter()
        .filter(|entry| {
            // Always keep functions - they're the contract's API
            if matches!(entry, ScSpecEntry::FunctionV0(_)) {
                return true;
            }
            // For all other entries (types, events), check if marker exists
            let hash = compute_marker_hash(entry);
            markers.contains(&hash)
        })
        .collect()
}

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
/// - All functions (`FunctionV0`)
/// - All events (`EventV0`)
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
        let inputs = input_types
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
        let fields = field_types
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

    // Helper to encode a marker (matches SDK's spec_marker.rs format)
    // Format: "SpEc" (4 bytes) + truncated SHA256 hash (8 bytes)
    fn encode_marker(entry: &ScSpecEntry) -> Vec<u8> {
        let hash = compute_marker_hash(entry);
        let mut buf = Vec::new();
        buf.extend_from_slice(&SPEC_MARKER_MAGIC);
        buf.extend_from_slice(&hash.0);
        buf
    }

    use stellar_xdr::curr::{ScSpecEventDataFormat, ScSpecEventV0};

    fn make_event(name: &str) -> ScSpecEntry {
        ScSpecEntry::EventV0(ScSpecEventV0 {
            doc: StringM::default(),
            lib: StringM::default(),
            name: name.try_into().unwrap(),
            prefix_topics: VecM::default(),
            params: VecM::default(),
            data_format: ScSpecEventDataFormat::SingleValue,
        })
    }

    #[test]
    fn test_compute_marker_hash() {
        let entry = make_struct("MyStruct", vec![("field", ScSpecTypeDef::U32)]);
        let hash = compute_marker_hash(&entry);

        // Hash should be 8 bytes
        assert_eq!(hash.0.len(), SPEC_MARKER_HASH_LEN);

        // Same entry produces same hash
        let hash2 = compute_marker_hash(&entry);
        assert_eq!(hash.0, hash2.0);

        // Different entry produces different hash
        let entry2 = make_struct("DifferentStruct", vec![("field", ScSpecTypeDef::U32)]);
        let hash3 = compute_marker_hash(&entry2);
        assert_ne!(hash.0, hash3.0);
    }

    #[test]
    fn test_encode_marker_format() {
        let entry = make_event("Transfer");
        let marker = encode_marker(&entry);

        // Marker should be 12 bytes: 4 (magic) + 8 (hash)
        assert_eq!(marker.len(), SPEC_MARKER_LEN);

        // First 4 bytes should be magic
        assert_eq!(&marker[..4], &SPEC_MARKER_MAGIC);
    }

    #[test]
    fn test_extract_markers_from_data() {
        let entry1 = make_event("Transfer");
        let entry2 = make_struct("MyStruct", vec![("field", ScSpecTypeDef::U32)]);

        let encoded1 = encode_marker(&entry1);
        let encoded2 = encode_marker(&entry2);

        // Concatenate markers with some padding
        let mut data = Vec::new();
        data.extend_from_slice(&[0u8; 16]); // Some leading bytes
        data.extend_from_slice(&encoded1);
        data.extend_from_slice(&[0u8; 8]); // Some padding
        data.extend_from_slice(&encoded2);
        data.extend_from_slice(&[0u8; 16]); // Some trailing bytes

        let mut found = HashSet::new();
        extract_markers_from_data(&data, &mut found);

        // Both markers should be found
        assert!(found.contains(&compute_marker_hash(&entry1)));
        assert!(found.contains(&compute_marker_hash(&entry2)));
    }

    #[test]
    fn test_filter_by_markers_keeps_used_events() {
        let transfer_event = make_event("Transfer");
        let mint_event = make_event("Mint");

        let entries = vec![
            make_function("foo", vec![ScSpecTypeDef::U32]),
            transfer_event.clone(),
            mint_event.clone(),
            make_event("Unused"),
        ];

        let mut markers = HashSet::new();
        markers.insert(compute_marker_hash(&transfer_event));
        markers.insert(compute_marker_hash(&mint_event));

        let filtered = filter_by_markers(entries, &markers);

        // Should have: 1 function + 2 used events
        assert_eq!(filtered.len(), 3);

        let event_names: Vec<_> = filtered
            .iter()
            .filter_map(|e| {
                if let ScSpecEntry::EventV0(event) = e {
                    Some(event.name.to_utf8_string_lossy())
                } else {
                    None
                }
            })
            .collect();

        assert!(event_names.contains(&"Transfer".to_string()));
        assert!(event_names.contains(&"Mint".to_string()));
        assert!(!event_names.contains(&"Unused".to_string()));
    }

    #[test]
    fn test_filter_by_markers_removes_all_events_if_no_markers() {
        let entries = vec![
            make_function("foo", vec![ScSpecTypeDef::U32]),
            make_event("Transfer"),
            make_event("Mint"),
        ];

        let markers = HashSet::new();

        let filtered = filter_by_markers(entries, &markers);

        // Should have: 1 function, 0 events
        assert_eq!(filtered.len(), 1);
        assert!(matches!(filtered[0], ScSpecEntry::FunctionV0(_)));
    }

    #[test]
    fn test_filter_by_markers_removes_all_if_no_markers() {
        let entries = vec![
            make_function("foo", vec![ScSpecTypeDef::U32]),
            make_struct("MyStruct", vec![("field", ScSpecTypeDef::U32)]),
            make_enum("MyEnum"),
            make_event("Unused"),
        ];

        let markers = HashSet::new(); // No markers

        let filtered = filter_by_markers(entries, &markers);

        // Should have: only functions (always kept), no types or events
        assert_eq!(filtered.len(), 1);
        assert!(filtered
            .iter()
            .all(|e| matches!(e, ScSpecEntry::FunctionV0(_))));
    }

    #[test]
    fn test_filter_by_markers_keeps_types_with_markers() {
        let used_struct = make_struct("UsedStruct", vec![("field", ScSpecTypeDef::U32)]);
        let used_enum = make_enum("UsedEnum");
        let used_event = make_event("UsedEvent");

        let entries = vec![
            make_function("foo", vec![ScSpecTypeDef::U32]),
            used_struct.clone(),
            make_struct("UnusedStruct", vec![("field", ScSpecTypeDef::U32)]),
            used_enum.clone(),
            make_enum("UnusedEnum"),
            used_event.clone(),
            make_event("UnusedEvent"),
        ];

        let mut markers = HashSet::new();
        markers.insert(compute_marker_hash(&used_struct));
        markers.insert(compute_marker_hash(&used_enum));
        markers.insert(compute_marker_hash(&used_event));

        let filtered = filter_by_markers(entries, &markers);

        // Should have: 1 function + 1 struct + 1 enum + 1 event
        assert_eq!(filtered.len(), 4);

        // Check specific entries
        let struct_names: Vec<_> = filtered
            .iter()
            .filter_map(|e| {
                if let ScSpecEntry::UdtStructV0(s) = e {
                    Some(s.name.to_utf8_string_lossy())
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(struct_names, vec!["UsedStruct"]);

        let enum_names: Vec<_> = filtered
            .iter()
            .filter_map(|e| {
                if let ScSpecEntry::UdtEnumV0(s) = e {
                    Some(s.name.to_utf8_string_lossy())
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(enum_names, vec!["UsedEnum"]);

        let event_names: Vec<_> = filtered
            .iter()
            .filter_map(|e| {
                if let ScSpecEntry::EventV0(s) = e {
                    Some(s.name.to_utf8_string_lossy())
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(event_names, vec!["UsedEvent"]);
    }
}
