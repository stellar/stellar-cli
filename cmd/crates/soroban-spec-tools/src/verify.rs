use std::collections::HashSet;

use stellar_xdr::curr::{
    ScSpecEntry, ScSpecTypeDef as ScType, ScSpecTypeUdt, ScSpecUdtUnionCaseTupleV0,
    ScSpecUdtUnionCaseV0,
};

use crate::{sanitize, Spec};

// Types defined in the Rust soroban-sdk that are referenced in contract specs
// but are never exported as UDT definitions, at least in current versions of
// the soroban-sdk.
const BUILTIN_UDT_NAMES: &[&str] = &[
    "Context",
    "ContractContext",
    "CreateContractHostFnContext",
    "CreateContractWithCtorHostFnContext",
    "SubContractInvocation",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpecWarning {
    UndefinedType { context: String, type_name: String },
    DuplicateEntry { name: String },
}

impl std::fmt::Display for SpecWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SpecWarning::UndefinedType { type_name, context } => write!(
                f,
                "type '{}' referenced by {} is not defined in the spec",
                sanitize(type_name),
                sanitize(context),
            ),
            SpecWarning::DuplicateEntry { name } => {
                write!(
                    f,
                    "spec entry '{}' is defined more than once",
                    sanitize(name)
                )
            }
        }
    }
}

fn entry_name(entry: &ScSpecEntry) -> String {
    match entry {
        ScSpecEntry::FunctionV0(x) => x.name.to_utf8_string_lossy(),
        ScSpecEntry::UdtStructV0(x) => x.name.to_utf8_string_lossy(),
        ScSpecEntry::UdtUnionV0(x) => x.name.to_utf8_string_lossy(),
        ScSpecEntry::UdtEnumV0(x) => x.name.to_utf8_string_lossy(),
        ScSpecEntry::UdtErrorEnumV0(x) => x.name.to_utf8_string_lossy(),
        ScSpecEntry::EventV0(x) => x.name.to_utf8_string_lossy(),
    }
}

impl Spec {
    /// Verify the spec is well-formed, returning warnings for any issues found.
    ///
    /// Checks performed:
    /// - Duplicate entries: entries that share a name with another entry.
    /// - Undefined types: UDT names referenced in function signatures, event
    ///   params, struct fields, or union cases that are not defined in the spec.
    pub fn verify(&self) -> Vec<SpecWarning> {
        let Some(entries) = &self.0 else {
            return vec![];
        };

        let mut warnings = Vec::new();

        // Collect all defined type names and detect duplicates.
        let mut seen: HashSet<String> = HashSet::new();
        let mut defined: HashSet<String> = HashSet::new();
        for entry in entries {
            let name = entry_name(entry);
            if !seen.insert(name.clone()) {
                warnings.push(SpecWarning::DuplicateEntry { name });
                continue;
            }
            match entry {
                ScSpecEntry::UdtStructV0(_)
                | ScSpecEntry::UdtUnionV0(_)
                | ScSpecEntry::UdtEnumV0(_)
                | ScSpecEntry::UdtErrorEnumV0(_) => {
                    defined.insert(name);
                }
                ScSpecEntry::FunctionV0(_) | ScSpecEntry::EventV0(_) => {}
            }
        }
        for name in BUILTIN_UDT_NAMES {
            defined.insert((*name).to_string());
        }

        // Walk every entry and flag any UDT references not in the defined set.
        for entry in entries {
            find_undefined_types(entry, &defined, &mut warnings);
        }
        warnings
    }
}

fn find_undefined_types(
    entry: &ScSpecEntry,
    defined: &HashSet<String>,
    warnings: &mut Vec<SpecWarning>,
) {
    fn check_type(
        context: &str,
        type_def: &ScType,
        defined: &HashSet<String>,
        warnings: &mut Vec<SpecWarning>,
    ) {
        for name in collect_udt_names(type_def) {
            if !defined.contains(&name) {
                warnings.push(SpecWarning::UndefinedType {
                    context: context.to_string(),
                    type_name: name,
                });
            }
        }
    }

    match entry {
        ScSpecEntry::FunctionV0(f) => {
            let fn_name = f.name.to_utf8_string_lossy();
            for input in f.inputs.iter() {
                let input_name = input.name.to_utf8_string_lossy();
                check_type(
                    &format!("function '{fn_name}' input '{input_name}'"),
                    &input.type_,
                    defined,
                    warnings,
                );
            }
            for output in f.outputs.iter() {
                check_type(
                    &format!("function '{fn_name}' output"),
                    output,
                    defined,
                    warnings,
                );
            }
        }
        ScSpecEntry::EventV0(e) => {
            let event_name = e.name.to_utf8_string_lossy();
            for param in e.params.iter() {
                let param_name = param.name.to_utf8_string_lossy();
                check_type(
                    &format!("event '{event_name}' param '{param_name}'"),
                    &param.type_,
                    defined,
                    warnings,
                );
            }
        }
        ScSpecEntry::UdtStructV0(s) => {
            let struct_name = s.name.to_utf8_string_lossy();
            for field in s.fields.iter() {
                let field_name = field.name.to_utf8_string_lossy();
                check_type(
                    &format!("struct '{struct_name}' field '{field_name}'"),
                    &field.type_,
                    defined,
                    warnings,
                );
            }
        }
        ScSpecEntry::UdtUnionV0(u) => {
            let union_name = u.name.to_utf8_string_lossy();
            for case in u.cases.iter() {
                if let ScSpecUdtUnionCaseV0::TupleV0(ScSpecUdtUnionCaseTupleV0 {
                    name,
                    type_,
                    ..
                }) = case
                {
                    let case_name = name.to_utf8_string_lossy();
                    for t in type_.iter() {
                        check_type(
                            &format!("union '{union_name}' case '{case_name}'"),
                            t,
                            defined,
                            warnings,
                        );
                    }
                }
            }
        }
        ScSpecEntry::UdtEnumV0(_) | ScSpecEntry::UdtErrorEnumV0(_) => {}
    }
}

fn collect_udt_names(type_def: &ScType) -> Vec<String> {
    match type_def {
        ScType::Udt(ScSpecTypeUdt { name }) => vec![name.to_utf8_string_lossy()],
        ScType::Vec(v) => collect_udt_names(&v.element_type),
        ScType::Option(o) => collect_udt_names(&o.value_type),
        ScType::Map(m) => {
            let mut names = collect_udt_names(&m.key_type);
            names.extend(collect_udt_names(&m.value_type));
            names
        }
        ScType::Result(r) => {
            let mut names = collect_udt_names(&r.ok_type);
            names.extend(collect_udt_names(&r.error_type));
            names
        }
        ScType::Tuple(t) => t.value_types.iter().flat_map(collect_udt_names).collect(),
        _ => vec![],
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use stellar_xdr::curr::{
        ScSpecEntry, ScSpecFunctionV0, ScSpecTypeDef as ScType, ScSpecTypeMap, ScSpecTypeOption,
        ScSpecTypeUdt, ScSpecTypeVec, ScSpecUdtStructV0, ScSymbol, StringM,
    };

    use super::SpecWarning;

    use crate::Spec;

    fn make_udt_type(name: &str) -> ScType {
        ScType::Udt(ScSpecTypeUdt {
            name: StringM::from_str(name).unwrap(),
        })
    }

    fn make_struct_entry(name: &str, field_types: Vec<(&str, ScType)>) -> ScSpecEntry {
        ScSpecEntry::UdtStructV0(ScSpecUdtStructV0 {
            doc: StringM::default(),
            lib: StringM::default(),
            name: StringM::from_str(name).unwrap(),
            fields: field_types
                .into_iter()
                .map(|(fname, ftype)| stellar_xdr::curr::ScSpecUdtStructFieldV0 {
                    doc: StringM::default(),
                    name: StringM::from_str(fname).unwrap(),
                    type_: ftype,
                })
                .collect::<Vec<_>>()
                .try_into()
                .unwrap(),
        })
    }

    fn make_fn_entry(name: &str, inputs: Vec<(&str, ScType)>, outputs: Vec<ScType>) -> ScSpecEntry {
        ScSpecEntry::FunctionV0(ScSpecFunctionV0 {
            doc: StringM::default(),
            name: ScSymbol(name.try_into().unwrap()),
            inputs: inputs
                .into_iter()
                .map(|(n, t)| stellar_xdr::curr::ScSpecFunctionInputV0 {
                    doc: StringM::default(),
                    name: StringM::from_str(n).unwrap(),
                    type_: t,
                })
                .collect::<Vec<_>>()
                .try_into()
                .unwrap(),
            outputs: outputs.try_into().unwrap(),
        })
    }

    #[test]
    fn test_verify_complete_spec_no_warnings() {
        let entries = vec![
            make_struct_entry("MyStruct", vec![("val", ScType::U32)]),
            make_fn_entry("do_thing", vec![("s", make_udt_type("MyStruct"))], vec![]),
        ];
        let spec = Spec::new(&entries);
        assert!(spec.verify().is_empty());
    }

    #[test]
    fn test_verify_missing_type_in_function() {
        let entries = vec![make_fn_entry(
            "do_thing",
            vec![("s", make_udt_type("Missing"))],
            vec![],
        )];
        let spec = Spec::new(&entries);
        let warnings = spec.verify();
        assert_eq!(warnings.len(), 1);
        assert!(matches!(
            &warnings[0],
            SpecWarning::UndefinedType { type_name, context }
                if type_name == "Missing" && context.contains("do_thing")
        ));
    }

    #[test]
    fn test_verify_builtin_type_ignored() {
        let entries = vec![make_fn_entry(
            "do_thing",
            vec![("ctx", make_udt_type("Context"))],
            vec![],
        )];
        let spec = Spec::new(&entries);
        assert!(spec.verify().is_empty());
    }

    #[test]
    fn test_verify_nested_type_in_vec() {
        let entries = vec![make_fn_entry(
            "list",
            vec![(
                "items",
                ScType::Vec(Box::new(ScSpecTypeVec {
                    element_type: Box::new(make_udt_type("Missing")),
                })),
            )],
            vec![],
        )];
        let spec = Spec::new(&entries);
        let warnings = spec.verify();
        assert_eq!(warnings.len(), 1);
        assert!(matches!(
            &warnings[0],
            SpecWarning::UndefinedType { type_name, .. } if type_name == "Missing"
        ));
    }

    #[test]
    fn test_verify_nested_type_in_map() {
        let entries = vec![make_fn_entry(
            "lookup",
            vec![(
                "m",
                ScType::Map(Box::new(ScSpecTypeMap {
                    key_type: Box::new(ScType::Symbol),
                    value_type: Box::new(make_udt_type("Missing")),
                })),
            )],
            vec![],
        )];
        let spec = Spec::new(&entries);
        let warnings = spec.verify();
        assert_eq!(warnings.len(), 1);
        assert!(matches!(
            &warnings[0],
            SpecWarning::UndefinedType { type_name, .. } if type_name == "Missing"
        ));
    }

    #[test]
    fn test_verify_nested_type_in_option() {
        let entries = vec![make_fn_entry(
            "maybe",
            vec![(
                "o",
                ScType::Option(Box::new(ScSpecTypeOption {
                    value_type: Box::new(make_udt_type("Missing")),
                })),
            )],
            vec![],
        )];
        let spec = Spec::new(&entries);
        let warnings = spec.verify();
        assert_eq!(warnings.len(), 1);
        assert!(matches!(
            &warnings[0],
            SpecWarning::UndefinedType { type_name, .. } if type_name == "Missing"
        ));
    }

    #[test]
    fn test_verify_struct_field_referencing_undefined_type() {
        let entries = vec![make_struct_entry(
            "Outer",
            vec![("inner", make_udt_type("Inner"))],
        )];
        let spec = Spec::new(&entries);
        let warnings = spec.verify();
        assert_eq!(warnings.len(), 1);
        assert!(matches!(
            &warnings[0],
            SpecWarning::UndefinedType { type_name, context }
                if type_name == "Inner" && context.contains("Outer")
        ));
    }

    #[test]
    fn test_verify_duplicate_entry() {
        let entries = vec![
            make_struct_entry("MyStruct", vec![("val", ScType::U32)]),
            make_struct_entry("MyStruct", vec![("other", ScType::Bool)]),
        ];
        let spec = Spec::new(&entries);
        let warnings = spec.verify();
        assert_eq!(warnings.len(), 1);
        assert_eq!(
            warnings[0],
            SpecWarning::DuplicateEntry {
                name: "MyStruct".to_string()
            }
        );
    }

    #[test]
    fn test_verify_duplicate_across_types() {
        let entries = vec![
            make_struct_entry("Foo", vec![("val", ScType::U32)]),
            make_fn_entry("Foo", vec![], vec![]),
        ];
        let spec = Spec::new(&entries);
        let warnings = spec.verify();
        assert_eq!(warnings.len(), 1);
        assert_eq!(
            warnings[0],
            SpecWarning::DuplicateEntry {
                name: "Foo".to_string()
            }
        );
    }

    #[test]
    fn undefined_type_display_strips_control_chars() {
        let w = SpecWarning::UndefinedType {
            type_name: "Evil\x1b[31m".into(),
            context: "function 'do\x1b[2J' input 'x'".into(),
        };
        crate::test_utils::assert_no_control_chars(&format!("{w}"));
    }

    #[test]
    fn duplicate_entry_display_strips_control_chars() {
        let w = SpecWarning::DuplicateEntry {
            name: "Foo\x1b[2J\x1b[H".into(),
        };
        crate::test_utils::assert_no_control_chars(&format!("{w}"));
    }
}
