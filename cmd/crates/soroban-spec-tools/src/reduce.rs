//! Reduce the fully qualified user-defined type names that appear in a contract
//! spec down to short, human-friendly names for display and code generation.
//!
//! Contract specs built with a soroban-sdk that names user-defined types by
//! their fully qualified Rust path (see
//! <https://github.com/stellar/rs-soroban-sdk/pull/1970>) carry names such as
//! `my_contract::inner::State`. Those names are unambiguous but noisy, and the
//! `::` separator is not a valid identifier, so the Rust and TypeScript binding
//! generators cannot use them verbatim. This module rewrites every user-defined
//! type name — both where the type is declared and everywhere it is referenced
//! — to its final path segment (`State`). When two distinct qualified names
//! reduce to the same short name they are disambiguated with a numeric suffix
//! (`State`, `State1`), and the collision is reported so a caller can warn.

use std::collections::{BTreeMap, HashSet};

use stellar_xdr::{
    ScSpecEntry, ScSpecEventV0, ScSpecFunctionV0, ScSpecTypeDef, ScSpecTypeMap, ScSpecTypeOption,
    ScSpecTypeResult, ScSpecTypeTuple, ScSpecTypeUdt, ScSpecTypeVec, ScSpecUdtEnumV0,
    ScSpecUdtErrorEnumV0, ScSpecUdtStructV0, ScSpecUdtUnionCaseV0, ScSpecUdtUnionV0,
};

/// A single user-defined type name that was rewritten during reduction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rename {
    /// The original, fully qualified name (e.g. `my_contract::inner::State`).
    pub from: String,
    /// The reduced name it was rewritten to (e.g. `State` or `State1`).
    pub to: String,
}

/// A group of distinct qualified names that share the same short name and so
/// had to be disambiguated with numeric suffixes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Collision {
    /// The short name they all reduced to before suffixing (e.g. `State`).
    pub short: String,
    /// The colliding members, each with the suffixed name it received.
    pub members: Vec<Rename>,
}

/// The record of what `reduce_udt_names` changed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Reduction {
    /// Every type whose name changed, in declaration order.
    pub renames: Vec<Rename>,
    /// The subset of renames that were forced to a numeric suffix because
    /// another qualified type reduced to the same short name.
    pub collisions: Vec<Collision>,
}

impl Reduction {
    /// Whether any name was rewritten.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.renames.is_empty()
    }
}

/// The final path segment of a qualified name, i.e. the part after the last
/// `::`. Names without a `::` are returned unchanged.
fn short_name(name: &str) -> &str {
    name.rsplit("::").next().unwrap_or(name)
}

/// Reduce the fully qualified user-defined type names in `spec` to short names,
/// returning the rewritten spec alongside a report of what changed. A spec that
/// already uses short names (no `::`) is returned unchanged with an empty
/// [`Reduction`].
#[must_use]
pub fn reduce_udt_names(spec: &[ScSpecEntry]) -> (Vec<ScSpecEntry>, Reduction) {
    let declared = declared_udt_names(spec);
    let (map, reduction) = build_mapping(&declared);

    let reduced = spec.iter().map(|e| rewrite_entry(e, &map)).collect();
    (reduced, reduction)
}

/// The names of every user-defined type declared in the spec, in declaration
/// order and de-duplicated.
fn declared_udt_names(spec: &[ScSpecEntry]) -> Vec<String> {
    let mut names = Vec::new();
    let mut seen = HashSet::new();
    let mut push = |name: String| {
        if seen.insert(name.clone()) {
            names.push(name);
        }
    };
    for entry in spec {
        match entry {
            ScSpecEntry::UdtStructV0(ScSpecUdtStructV0 { name, .. })
            | ScSpecEntry::UdtUnionV0(ScSpecUdtUnionV0 { name, .. })
            | ScSpecEntry::UdtEnumV0(ScSpecUdtEnumV0 { name, .. })
            | ScSpecEntry::UdtErrorEnumV0(ScSpecUdtErrorEnumV0 { name, .. }) => {
                push(name.to_utf8_string_lossy());
            }
            ScSpecEntry::FunctionV0(_) | ScSpecEntry::EventV0(_) => {}
        }
    }
    names
}

/// Build the rename map from qualified name to reduced name, grouping by short
/// name so collisions can be disambiguated deterministically.
fn build_mapping(declared: &[String]) -> (BTreeMap<String, String>, Reduction) {
    // Group the declared names by their short name. `BTreeMap`/sorted members
    // keep suffix assignment stable across runs.
    let mut groups: BTreeMap<&str, Vec<&String>> = BTreeMap::new();
    for name in declared {
        groups.entry(short_name(name)).or_default().push(name);
    }
    for members in groups.values_mut() {
        members.sort();
    }

    let mut map = BTreeMap::new();
    let mut renames = Vec::new();
    let mut collisions = Vec::new();
    // Every reduced name handed out, so a numeric suffix never lands on a name
    // already taken by another (possibly unrelated) type.
    let mut used: HashSet<String> = HashSet::new();

    for (short, members) in &groups {
        let collision = members.len() > 1;
        let mut collision_members = Vec::new();
        for (i, full) in members.iter().enumerate() {
            let mut n = i;
            let mut candidate = if i == 0 {
                (*short).to_string()
            } else {
                format!("{short}{n}")
            };
            while used.contains(&candidate) {
                n += 1;
                candidate = format!("{short}{n}");
            }
            used.insert(candidate.clone());
            map.insert((*full).clone(), candidate.clone());
            if **full != candidate {
                let rename = Rename {
                    from: (*full).clone(),
                    to: candidate.clone(),
                };
                renames.push(rename.clone());
                if collision {
                    collision_members.push(rename);
                }
            } else if collision {
                collision_members.push(Rename {
                    from: (*full).clone(),
                    to: candidate,
                });
            }
        }
        if collision {
            collisions.push(Collision {
                short: (*short).to_string(),
                members: collision_members,
            });
        }
    }

    // Report renames in the spec's declaration order rather than sorted order.
    renames.sort_by_key(|r| {
        declared
            .iter()
            .position(|d| *d == r.from)
            .unwrap_or(usize::MAX)
    });

    (
        map,
        Reduction {
            renames,
            collisions,
        },
    )
}

/// Look up the reduced name for a referenced type. A reference to a declared
/// type resolves through the map; a stray qualified name that was never
/// declared still gets its `::` stripped so no invalid identifier leaks into
/// the output.
fn reduced_ref(name: &str, map: &BTreeMap<String, String>) -> String {
    map.get(name)
        .cloned()
        .unwrap_or_else(|| short_name(name).to_string())
}

fn rewrite_entry(entry: &ScSpecEntry, map: &BTreeMap<String, String>) -> ScSpecEntry {
    match entry {
        ScSpecEntry::UdtStructV0(s) => {
            let mut s = s.clone();
            s.name = rename_udt(&s.name.to_utf8_string_lossy(), map);
            s.fields = s
                .fields
                .iter()
                .map(|f| {
                    let mut f = f.clone();
                    f.type_ = rewrite_type(&f.type_, map);
                    f
                })
                .collect::<Vec<_>>()
                .try_into()
                .unwrap_or(s.fields);
            ScSpecEntry::UdtStructV0(s)
        }
        ScSpecEntry::UdtUnionV0(u) => {
            let mut u = u.clone();
            u.name = rename_udt(&u.name.to_utf8_string_lossy(), map);
            u.cases = u
                .cases
                .iter()
                .map(|c| rewrite_union_case(c, map))
                .collect::<Vec<_>>()
                .try_into()
                .unwrap_or(u.cases);
            ScSpecEntry::UdtUnionV0(u)
        }
        ScSpecEntry::UdtEnumV0(e) => {
            let mut e = e.clone();
            e.name = rename_udt(&e.name.to_utf8_string_lossy(), map);
            ScSpecEntry::UdtEnumV0(e)
        }
        ScSpecEntry::UdtErrorEnumV0(e) => {
            let mut e = e.clone();
            e.name = rename_udt(&e.name.to_utf8_string_lossy(), map);
            ScSpecEntry::UdtErrorEnumV0(e)
        }
        ScSpecEntry::FunctionV0(f) => {
            let ScSpecFunctionV0 {
                doc,
                name,
                inputs,
                outputs,
            } = f;
            let mut f = ScSpecFunctionV0 {
                doc: doc.clone(),
                name: name.clone(),
                inputs: inputs.clone(),
                outputs: outputs.clone(),
            };
            f.inputs = f
                .inputs
                .iter()
                .map(|i| {
                    let mut i = i.clone();
                    i.type_ = rewrite_type(&i.type_, map);
                    i
                })
                .collect::<Vec<_>>()
                .try_into()
                .unwrap_or(f.inputs);
            f.outputs = f
                .outputs
                .iter()
                .map(|o| rewrite_type(o, map))
                .collect::<Vec<_>>()
                .try_into()
                .unwrap_or(f.outputs);
            ScSpecEntry::FunctionV0(f)
        }
        ScSpecEntry::EventV0(e) => {
            let ScSpecEventV0 { .. } = e;
            let mut e = e.clone();
            e.params = e
                .params
                .iter()
                .map(|p| {
                    let mut p = p.clone();
                    p.type_ = rewrite_type(&p.type_, map);
                    p
                })
                .collect::<Vec<_>>()
                .try_into()
                .unwrap_or(e.params);
            ScSpecEntry::EventV0(e)
        }
    }
}

fn rewrite_union_case(
    case: &ScSpecUdtUnionCaseV0,
    map: &BTreeMap<String, String>,
) -> ScSpecUdtUnionCaseV0 {
    match case {
        ScSpecUdtUnionCaseV0::VoidV0(_) => case.clone(),
        ScSpecUdtUnionCaseV0::TupleV0(t) => {
            let mut t = t.clone();
            t.type_ = t
                .type_
                .iter()
                .map(|ty| rewrite_type(ty, map))
                .collect::<Vec<_>>()
                .try_into()
                .unwrap_or(t.type_);
            ScSpecUdtUnionCaseV0::TupleV0(t)
        }
    }
}

/// Rewrite a type, recursing into every place a user-defined type can be
/// referenced.
fn rewrite_type(ty: &ScSpecTypeDef, map: &BTreeMap<String, String>) -> ScSpecTypeDef {
    match ty {
        ScSpecTypeDef::Udt(ScSpecTypeUdt { name }) => ScSpecTypeDef::Udt(ScSpecTypeUdt {
            name: rename_udt(&name.to_utf8_string_lossy(), map),
        }),
        ScSpecTypeDef::Option(o) => ScSpecTypeDef::Option(Box::new(ScSpecTypeOption {
            value_type: Box::new(rewrite_type(&o.value_type, map)),
        })),
        ScSpecTypeDef::Result(r) => ScSpecTypeDef::Result(Box::new(ScSpecTypeResult {
            ok_type: Box::new(rewrite_type(&r.ok_type, map)),
            error_type: Box::new(rewrite_type(&r.error_type, map)),
        })),
        ScSpecTypeDef::Vec(v) => ScSpecTypeDef::Vec(Box::new(ScSpecTypeVec {
            element_type: Box::new(rewrite_type(&v.element_type, map)),
        })),
        ScSpecTypeDef::Map(m) => ScSpecTypeDef::Map(Box::new(ScSpecTypeMap {
            key_type: Box::new(rewrite_type(&m.key_type, map)),
            value_type: Box::new(rewrite_type(&m.value_type, map)),
        })),
        ScSpecTypeDef::Tuple(t) => ScSpecTypeDef::Tuple(Box::new(ScSpecTypeTuple {
            value_types: t
                .value_types
                .iter()
                .map(|vt| rewrite_type(vt, map))
                .collect::<Vec<_>>()
                .try_into()
                .unwrap_or_else(|_| t.value_types.clone()),
        })),
        other => other.clone(),
    }
}

/// Rewrite a declaration or reference name to its reduced form as a
/// `StringM<256>`. The reduced name is never longer than the original, so it
/// always fits.
fn rename_udt(
    name: &str,
    map: &BTreeMap<String, String>,
) -> stellar_xdr::StringM<{ crate::UDT_NAME_LIMIT }> {
    reduced_ref(name, map)
        .try_into()
        .unwrap_or_else(|_| name.try_into().unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use stellar_xdr::{
        ScSpecFunctionInputV0, ScSpecTypeUdt, ScSpecUdtEnumV0, ScSpecUdtStructFieldV0,
        ScSpecUdtStructV0, StringM, VecM,
    };

    fn udt(name: &str) -> ScSpecTypeDef {
        ScSpecTypeDef::Udt(ScSpecTypeUdt {
            name: name.try_into().unwrap(),
        })
    }

    fn struct_entry(name: &str, field_type: ScSpecTypeDef) -> ScSpecEntry {
        ScSpecEntry::UdtStructV0(ScSpecUdtStructV0 {
            doc: StringM::default(),
            lib: StringM::default(),
            name: name.try_into().unwrap(),
            fields: vec![ScSpecUdtStructFieldV0 {
                doc: StringM::default(),
                name: "f".try_into().unwrap(),
                type_: field_type,
            }]
            .try_into()
            .unwrap(),
        })
    }

    fn enum_entry(name: &str) -> ScSpecEntry {
        ScSpecEntry::UdtEnumV0(ScSpecUdtEnumV0 {
            doc: StringM::default(),
            lib: StringM::default(),
            name: name.try_into().unwrap(),
            cases: VecM::default(),
        })
    }

    fn fn_entry(name: &str, input_type: ScSpecTypeDef) -> ScSpecEntry {
        ScSpecEntry::FunctionV0(ScSpecFunctionV0 {
            doc: StringM::default(),
            name: name.try_into().unwrap(),
            inputs: vec![ScSpecFunctionInputV0 {
                doc: StringM::default(),
                name: "arg".try_into().unwrap(),
                type_: input_type,
            }]
            .try_into()
            .unwrap(),
            outputs: VecM::default(),
        })
    }

    fn entry_name(entry: &ScSpecEntry) -> String {
        match entry {
            ScSpecEntry::UdtStructV0(s) => s.name.to_utf8_string_lossy(),
            ScSpecEntry::UdtUnionV0(u) => u.name.to_utf8_string_lossy(),
            ScSpecEntry::UdtEnumV0(e) => e.name.to_utf8_string_lossy(),
            ScSpecEntry::UdtErrorEnumV0(e) => e.name.to_utf8_string_lossy(),
            _ => String::new(),
        }
    }

    #[test]
    fn no_qualified_names_is_a_noop() {
        let spec = vec![enum_entry("State"), struct_entry("Point", udt("State"))];
        let (reduced, report) = reduce_udt_names(&spec);
        assert!(report.is_empty());
        assert_eq!(reduced, spec);
    }

    #[test]
    fn shortens_and_rewrites_references() {
        let spec = vec![
            enum_entry("my_contract::inner::State"),
            fn_entry("run", udt("my_contract::inner::State")),
        ];
        let (reduced, report) = reduce_udt_names(&spec);

        assert_eq!(entry_name(&reduced[0]), "State");
        // The reference inside the function input is rewritten too.
        let ScSpecEntry::FunctionV0(f) = &reduced[1] else {
            panic!("expected function")
        };
        assert_eq!(f.inputs[0].type_, udt("State"));

        assert_eq!(report.renames.len(), 1);
        assert_eq!(report.renames[0].from, "my_contract::inner::State");
        assert_eq!(report.renames[0].to, "State");
        assert!(report.collisions.is_empty());
    }

    #[test]
    fn disambiguates_collisions_with_suffixes() {
        let spec = vec![
            enum_entry("my_contract::a::Status"),
            enum_entry("my_contract::b::Status"),
            fn_entry("run", udt("my_contract::b::Status")),
        ];
        let (reduced, report) = reduce_udt_names(&spec);

        // Deterministic: `a::Status` sorts first and keeps the bare name.
        assert_eq!(entry_name(&reduced[0]), "Status");
        assert_eq!(entry_name(&reduced[1]), "Status1");
        // The reference to `b::Status` follows its rename to `Status1`.
        let ScSpecEntry::FunctionV0(f) = &reduced[2] else {
            panic!("expected function")
        };
        assert_eq!(f.inputs[0].type_, udt("Status1"));

        assert_eq!(report.collisions.len(), 1);
        let collision = &report.collisions[0];
        assert_eq!(collision.short, "Status");
        assert_eq!(collision.members.len(), 2);
        assert_eq!(collision.members[0].to, "Status");
        assert_eq!(collision.members[1].to, "Status1");
    }
}
