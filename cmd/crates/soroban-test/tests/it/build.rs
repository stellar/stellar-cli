use assert_fs::TempDir;
use fs_extra::dir::CopyOptions;
use predicates::prelude::{predicate, PredicateBooleanExt};
use shell_escape::escape;
use soroban_cli::xdr::{Limited, Limits, ReadXdr, ScMetaEntry, ScMetaV0, ScSpecEntry};
use soroban_spec_tools::contract::Spec;
use soroban_test::TestEnv;
use std::env;
use std::io::Cursor;
use std::path::{Path, PathBuf};

#[test]
fn build_all() {
    let sandbox = TestEnv::default();
    let cargo_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_path = cargo_dir.join("tests/fixtures/workspace/");
    let expected = format!(
        "cargo rustc {} --crate-type=cdylib --target=wasm32v1-none --release
cargo rustc {} --crate-type=cdylib --target=wasm32v1-none --release
cargo rustc {} --crate-type=cdylib --target=wasm32v1-none --release",
        manifest_path_arg(&add_path()),
        manifest_path_arg(&call_path()),
        manifest_path_arg(&add2_path()),
    );
    sandbox
        .new_assert_cmd("contract")
        .current_dir(fixture_path)
        .arg("build")
        .arg("--print-commands-only")
        .assert()
        .success()
        .stdout(predicate::eq(with_flags(expected.as_str())));
}

#[test]
fn build_package_by_name() {
    let sandbox = TestEnv::default();
    let cargo_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_path = cargo_dir.join("tests/fixtures/workspace/");
    let expected = format!(
        "cargo rustc {} --crate-type=cdylib --target=wasm32v1-none --release",
        manifest_path_arg(&add_path()),
    );
    sandbox
        .new_assert_cmd("contract")
        .current_dir(fixture_path)
        .arg("build")
        .arg("--print-commands-only")
        .arg("--package=add")
        .assert()
        .success()
        .stdout(predicate::eq(with_flags(expected.as_str())));
}

#[test]
fn build_package_by_current_dir() {
    let sandbox = TestEnv::default();
    let cargo_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_path = cargo_dir.join("tests/fixtures/workspace/contracts/add");
    sandbox
        .new_assert_cmd("contract")
        .current_dir(fixture_path)
        .arg("build")
        .arg("--print-commands-only")
        .assert()
        .success()
        .stdout(predicate::eq(
            with_flags("cargo rustc --manifest-path=Cargo.toml --crate-type=cdylib --target=wasm32v1-none --release"),
        ));
}

// `--env` is repeatable and sets env vars on the local cargo process; they
// surface in the printed command in --print-commands-only.
#[test]
fn build_with_env_vars() {
    let sandbox = TestEnv::default();
    let cargo_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_path = cargo_dir.join("tests/fixtures/workspace/contracts/add");
    sandbox
        .new_assert_cmd("contract")
        .current_dir(fixture_path)
        .arg("build")
        .arg("--print-commands-only")
        .arg("--env")
        .arg("FOO=bar")
        .arg("--env")
        .arg("BAZ=qux")
        .assert()
        .success()
        .stdout(predicate::str::contains("FOO=bar").and(predicate::str::contains("BAZ=qux")));
}

// An invalid `--env` name is rejected before building.
#[test]
fn build_rejects_invalid_env_name() {
    let sandbox = TestEnv::default();
    let cargo_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_path = cargo_dir.join("tests/fixtures/workspace/contracts/add");
    sandbox
        .new_assert_cmd("contract")
        .current_dir(fixture_path)
        .arg("build")
        .arg("--print-commands-only")
        .arg("--env")
        .arg("1FOO=bar")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "not a valid environment variable name",
        ));
}

#[test]
fn build_with_locked() {
    let sandbox = TestEnv::default();
    let cargo_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_path = cargo_dir.join("tests/fixtures/workspace/contracts/add");
    sandbox
        .new_assert_cmd("contract")
        .current_dir(fixture_path)
        .arg("build")
        .arg("--print-commands-only")
        .arg("--locked")
        .assert()
        .success()
        .stdout(predicate::eq(
            with_flags("cargo rustc --locked --manifest-path=Cargo.toml --crate-type=cdylib --target=wasm32v1-none --release"),
        ));
}

#[test]
fn build_no_package_found() {
    let sandbox = TestEnv::default();
    let cargo_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_path = cargo_dir.join("tests/fixtures/workspace/");
    sandbox
        .new_assert_cmd("contract")
        .current_dir(fixture_path)
        .arg("build")
        .arg("--print-commands-only")
        .arg("--package=nopkgwiththisname")
        .assert()
        .failure()
        .stderr(predicate::eq(
            "\
❌ error: package nopkgwiththisname not found
",
        ));
}

#[test]
fn build_all_when_in_non_package_directory() {
    let sandbox = TestEnv::default();
    let cargo_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_path = cargo_dir.join("tests/fixtures/workspace/contracts/add/src/");
    let expected = format!(
        "cargo rustc {} --crate-type=cdylib --target=wasm32v1-none --release",
        manifest_path_arg(&parent_path()),
    );

    sandbox
        .new_assert_cmd("contract")
        .current_dir(fixture_path)
        .arg("build")
        .arg("--print-commands-only")
        .assert()
        .success()
        .stdout(predicate::eq(with_flags(expected.as_str())));
}

#[test]
fn build_default_members() {
    let sandbox = TestEnv::default();
    let cargo_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_path = cargo_dir.join("tests/fixtures/workspace-with-default-members/");
    let expected = format!(
        "cargo rustc {} --crate-type=cdylib --target=wasm32v1-none --release",
        manifest_path_arg(&add_path()),
    );

    sandbox
        .new_assert_cmd("contract")
        .current_dir(fixture_path)
        .arg("build")
        .arg("--print-commands-only")
        .assert()
        .success()
        .stdout(predicate::eq(with_flags(expected.as_str())));
}

#[test]
fn build_with_metadata_rewrite() {
    let sandbox = TestEnv::default();
    let outdir = sandbox.dir().join("out");
    let cargo_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_path = cargo_dir.join("tests/fixtures/workspace");
    let temp = TempDir::new().unwrap();
    let dir_path = temp.path();
    fs_extra::dir::copy(fixture_path, dir_path, &CopyOptions::new()).unwrap();
    let dir_path = dir_path.join("workspace").join("contracts").join("add");

    sandbox
        .new_assert_cmd("contract")
        .current_dir(&dir_path)
        .arg("build")
        .arg("--meta")
        .arg("contract meta=added on build")
        .arg("--out-dir")
        .arg(&outdir)
        .assert()
        .success();

    sandbox
        .new_assert_cmd("contract")
        .current_dir(&dir_path)
        .arg("build")
        .arg("--meta")
        .arg("meta_replaced=some_new_meta")
        .arg("--out-dir")
        .arg(&outdir)
        .assert()
        .success();

    let entries = get_entries(&dir_path, &outdir);

    // Filter out CLI version for comparison
    let filtered_entries: Vec<_> = entries
        .into_iter()
        .filter(|entry| !matches!(entry, ScMetaEntry::ScMetaV0(ScMetaV0 { key, .. }) if key.to_string() == "cliver"))
        .collect();

    let expected_entries = vec![
        ScMetaEntry::ScMetaV0(ScMetaV0 {
            key: "Description".try_into().unwrap(),
            val: "A test add contract".try_into().unwrap(),
        }),
        ScMetaEntry::ScMetaV0(ScMetaV0 {
            key: "meta_replaced".try_into().unwrap(),
            val: "some_new_meta".try_into().unwrap(),
        }),
    ];

    assert_eq!(filtered_entries, expected_entries);
}

#[test]
fn build_with_metadata_diff_dir() {
    let sandbox = TestEnv::default();
    let outdir1 = sandbox.dir().join("out-1");
    let outdir2 = sandbox.dir().join("out-2");
    let cargo_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_path = cargo_dir.join("tests/fixtures/workspace");
    let temp = TempDir::new().unwrap();
    let dir_path = temp.path();
    fs_extra::dir::copy(fixture_path, dir_path, &CopyOptions::new()).unwrap();
    let dir_path = dir_path.join("workspace").join("contracts").join("add");

    sandbox
        .new_assert_cmd("contract")
        .current_dir(&dir_path)
        .arg("build")
        .arg("--meta")
        .arg("contract meta=added on build")
        .arg("--out-dir")
        .arg(&outdir1)
        .assert()
        .success();

    sandbox
        .new_assert_cmd("contract")
        .current_dir(&dir_path)
        .arg("build")
        .arg("--meta")
        .arg("meta_replaced=some_new_meta")
        .arg("--out-dir")
        .arg(&outdir2)
        .assert()
        .success();

    let entries_dir1 = get_entries(&dir_path, &outdir1);

    let entries_dir2 = get_entries(&dir_path, &outdir2);

    // Filter out CLI version for comparison
    let filtered_entries_dir1: Vec<_> = entries_dir1
        .into_iter()
        .filter(|entry| !matches!(entry, ScMetaEntry::ScMetaV0(ScMetaV0 { key, .. }) if key.to_string() == "cliver"))
        .collect();

    let filtered_entries_dir2: Vec<_> = entries_dir2
        .into_iter()
        .filter(|entry| !matches!(entry, ScMetaEntry::ScMetaV0(ScMetaV0 { key, .. }) if key.to_string() == "cliver"))
        .collect();

    let expected_entries_dir1 = vec![
        ScMetaEntry::ScMetaV0(ScMetaV0 {
            key: "Description".try_into().unwrap(),
            val: "A test add contract".try_into().unwrap(),
        }),
        ScMetaEntry::ScMetaV0(ScMetaV0 {
            key: "contract meta".try_into().unwrap(),
            val: "added on build".try_into().unwrap(),
        }),
    ];

    let expected_entries_dir2 = vec![
        ScMetaEntry::ScMetaV0(ScMetaV0 {
            key: "Description".try_into().unwrap(),
            val: "A test add contract".try_into().unwrap(),
        }),
        ScMetaEntry::ScMetaV0(ScMetaV0 {
            key: "meta_replaced".try_into().unwrap(),
            val: "some_new_meta".try_into().unwrap(),
        }),
    ];

    assert_eq!(filtered_entries_dir1, expected_entries_dir1);
    assert_eq!(filtered_entries_dir2, expected_entries_dir2);
}

fn build_spec_shaking_fixture() -> (Vec<ScSpecEntry>, Vec<ScMetaEntry>) {
    let sandbox = TestEnv::default();
    let outdir = sandbox.dir().join("out");
    let cargo_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_path = cargo_dir.join("tests/fixtures/workspace-with-spec-shaking");
    let temp = TempDir::new().unwrap();
    let dir_path = temp.path();
    fs_extra::dir::copy(fixture_path, dir_path, &CopyOptions::new()).unwrap();
    let dir_path = dir_path.join("workspace-with-spec-shaking");

    sandbox
        .new_assert_cmd("contract")
        .current_dir(&dir_path)
        .arg("build")
        .arg("--out-dir")
        .arg(&outdir)
        .assert()
        .success();

    let wasm_path = dir_path.join(&outdir).join("shaking.wasm");
    let wasm = std::fs::read(wasm_path).unwrap();
    let spec = Spec::new(&wasm).unwrap();
    (spec.spec, spec.meta)
}

fn spec_entry_name(entry: &ScSpecEntry) -> String {
    match entry {
        ScSpecEntry::FunctionV0(f) => f.name.to_utf8_string_lossy(),
        ScSpecEntry::UdtStructV0(s) => s.name.to_utf8_string_lossy(),
        ScSpecEntry::UdtUnionV0(u) => u.name.to_utf8_string_lossy(),
        ScSpecEntry::UdtEnumV0(e) => e.name.to_utf8_string_lossy(),
        ScSpecEntry::UdtErrorEnumV0(e) => e.name.to_utf8_string_lossy(),
        ScSpecEntry::EventV0(e) => e.name.to_utf8_string_lossy(),
    }
}

#[test]
fn build_with_spec_shaking_filters_unused_types() {
    let (spec, _meta) = build_spec_shaking_fixture();
    let names: Vec<String> = spec.iter().map(spec_entry_name).collect();

    // All functions should be present
    assert!(
        names.contains(&"use_struct".to_string()),
        "use_struct function should be present"
    );
    assert!(
        names.contains(&"use_enum".to_string()),
        "use_enum function should be present"
    );
    assert!(
        names.contains(&"emit_event".to_string()),
        "emit_event function should be present"
    );
    assert!(
        names.contains(&"hello".to_string()),
        "hello function should be present"
    );

    // Used types should be present
    assert!(
        names.contains(&"UsedStruct".to_string()),
        "UsedStruct should be present"
    );
    assert!(
        names.contains(&"UsedEnum".to_string()),
        "UsedEnum should be present"
    );

    // Unused types should be removed
    assert!(
        !names.contains(&"UnusedStruct".to_string()),
        "UnusedStruct should be removed"
    );
    assert!(
        !names.contains(&"UnusedEnum".to_string()),
        "UnusedEnum should be removed"
    );
}

#[test]
fn build_with_spec_shaking_filters_unused_events() {
    let (spec, _meta) = build_spec_shaking_fixture();
    let names: Vec<String> = spec.iter().map(spec_entry_name).collect();

    // Used event should be present
    assert!(
        names.contains(&"UsedEvent".to_string()),
        "UsedEvent should be present"
    );

    // Unused event should be removed
    assert!(
        !names.contains(&"UnusedEvent".to_string()),
        "UnusedEvent should be removed"
    );
}

#[test]
fn build_with_spec_shaking_preserves_all_functions() {
    let (spec, _meta) = build_spec_shaking_fixture();
    let function_names: Vec<String> = spec
        .iter()
        .filter(|e| matches!(e, ScSpecEntry::FunctionV0(_)))
        .map(spec_entry_name)
        .collect();

    assert!(function_names.contains(&"use_struct".to_string()));
    assert!(function_names.contains(&"use_enum".to_string()));
    assert!(function_names.contains(&"emit_event".to_string()));
    assert!(function_names.contains(&"hello".to_string()));
    assert_eq!(function_names.len(), 4, "expected exactly 4 functions");
}

#[test]
fn filter_and_dedup_spec_removes_duplicates() {
    use soroban_cli::commands::contract::build::filter_and_dedup_spec;
    use soroban_cli::xdr::{
        ReadXdr, ScSpecEntry, ScSpecFunctionInputV0, ScSpecFunctionV0, ScSpecTypeDef,
        ScSpecUdtStructFieldV0, ScSpecUdtStructV0, StringM, VecM,
    };

    let func = ScSpecEntry::FunctionV0(ScSpecFunctionV0 {
        doc: StringM::default(),
        name: "hello".try_into().unwrap(),
        inputs: vec![ScSpecFunctionInputV0 {
            doc: StringM::default(),
            name: "arg0".try_into().unwrap(),
            type_: ScSpecTypeDef::U32,
        }]
        .try_into()
        .unwrap(),
        outputs: VecM::default(),
    });

    let used_struct = ScSpecEntry::UdtStructV0(ScSpecUdtStructV0 {
        doc: StringM::default(),
        lib: StringM::default(),
        name: "MyStruct".try_into().unwrap(),
        fields: vec![ScSpecUdtStructFieldV0 {
            doc: StringM::default(),
            name: "field".try_into().unwrap(),
            type_: ScSpecTypeDef::U32,
        }]
        .try_into()
        .unwrap(),
    });

    // Build markers for the struct so it passes the filter
    let mut markers = std::collections::HashSet::new();
    markers.insert(soroban_spec::shaking::generate_marker_for_entry(
        &used_struct,
    ));

    // Input: function appears twice, struct appears three times
    let entries = vec![
        func.clone(),
        func.clone(),
        used_struct.clone(),
        used_struct.clone(),
        used_struct.clone(),
    ];

    let result_xdr = filter_and_dedup_spec(entries, &markers).unwrap();

    // Parse back the entries from the XDR
    let result_entries: Vec<ScSpecEntry> =
        ScSpecEntry::read_xdr_iter(&mut Limited::new(Cursor::new(result_xdr), Limits::none()))
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

    // Should have exactly 1 function + 1 struct, no duplicates
    assert_eq!(
        result_entries.len(),
        2,
        "expected 2 entries (1 function + 1 struct), got {}: {:?}",
        result_entries.len(),
        result_entries
            .iter()
            .map(spec_entry_name)
            .collect::<Vec<_>>()
    );
    assert_eq!(spec_entry_name(&result_entries[0]), "hello");
    assert_eq!(spec_entry_name(&result_entries[1]), "MyStruct");
}

#[test]
fn build_with_spec_shaking_has_feature_meta() {
    let (_spec, meta) = build_spec_shaking_fixture();

    let version = soroban_spec::shaking::spec_shaking_version_for_meta(&meta);

    assert_eq!(
        version, 2,
        "contractmeta should indicate spec shaking version 2"
    );
}

#[test]
fn build_without_spec_shaking_preserves_all_entries() {
    let sandbox = TestEnv::default();
    let outdir = sandbox.dir().join("out");
    let cargo_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_path = cargo_dir.join("tests/fixtures/workspace");
    let temp = TempDir::new().unwrap();
    let dir_path = temp.path();
    fs_extra::dir::copy(fixture_path, dir_path, &CopyOptions::new()).unwrap();
    let dir_path = dir_path.join("workspace").join("contracts").join("add");

    sandbox
        .new_assert_cmd("contract")
        .current_dir(&dir_path)
        .arg("build")
        .arg("--out-dir")
        .arg(&outdir)
        .assert()
        .success();

    let wasm_path = dir_path.join(&outdir).join("add.wasm");
    let wasm = std::fs::read(wasm_path).unwrap();
    let spec = Spec::new(&wasm).unwrap();

    // Without spec shaking, all spec entries should be preserved.
    // The "add" contract should have at least its function(s).
    let function_names: Vec<String> = spec
        .spec
        .iter()
        .filter(|e| matches!(e, ScSpecEntry::FunctionV0(_)))
        .map(spec_entry_name)
        .collect();
    assert!(
        !function_names.is_empty(),
        "functions should be preserved without spec shaking"
    );

    // Verify no rssdk_spec_shaking meta entry exists (no spec shaking support)
    let has_feature_meta = spec.meta.iter().any(|entry| {
        matches!(
            entry,
            ScMetaEntry::ScMetaV0(ScMetaV0 { key, .. })
                if key.to_string() == "rssdk_spec_shaking"
        )
    });
    assert!(
        !has_feature_meta,
        "workspace fixture should not have rssdk_spec_shaking meta"
    );
}

#[test]
fn replace_custom_section_replaces_and_consolidates() {
    use soroban_spec_tools::wasm::replace_custom_section;
    use wasm_encoder::{CustomSection, Module};

    // Build a minimal WASM with two custom sections with the same name
    let mut module = Module::new();
    module.section(&CustomSection {
        name: "test_section".into(),
        data: b"original_content_1".as_slice().into(),
    });
    module.section(&CustomSection {
        name: "test_section".into(),
        data: b"original_content_2".as_slice().into(),
    });
    module.section(&CustomSection {
        name: "other_section".into(),
        data: b"other_data".as_slice().into(),
    });
    let wasm = module.finish();

    // Replace the custom section
    let new_content = b"replaced_content";
    let result = replace_custom_section(&wasm, "test_section", new_content).unwrap();

    // Parse the result and verify
    let parser = wasmparser::Parser::new(0);
    let mut test_sections = Vec::new();
    let mut other_sections = Vec::new();
    for payload in parser.parse_all(&result) {
        let payload = payload.unwrap();
        if let wasmparser::Payload::CustomSection(section) = payload {
            if section.name() == "test_section" {
                test_sections.push(section.data().to_vec());
            } else if section.name() == "other_section" {
                other_sections.push(section.data().to_vec());
            }
        }
    }

    // Multiple sections consolidated into one
    assert_eq!(
        test_sections.len(),
        1,
        "should have exactly one test_section after replacement"
    );
    assert_eq!(test_sections[0], new_content, "content should be replaced");

    // Other section should be preserved
    assert_eq!(other_sections.len(), 1, "other_section should be preserved");
    assert_eq!(
        other_sections[0], b"other_data",
        "other_section content should be unchanged"
    );
}

fn get_entries(fixture_path: &Path, outdir: &Path) -> Vec<ScMetaEntry> {
    // verify that the metadata added in the contract code via contractmetadata! macro is present
    // as well as the meta that is included on build
    let wasm_path = fixture_path.join(outdir).join("add.wasm");
    let wasm = std::fs::read(wasm_path).unwrap();
    let spec = Spec::new(&wasm).unwrap();
    let meta = spec.meta_base64.unwrap();
    ScMetaEntry::read_xdr_base64_iter(&mut Limited::new(
        Cursor::new(meta.as_bytes()),
        Limits::none(),
    ))
    .filter(|entry| match entry {
        // Ignore the meta entries that the SDK embeds that capture the SDK and
        // Rust version, since these will change often and are not really
        // relevant to this test.
        Ok(ScMetaEntry::ScMetaV0(ScMetaV0 { key, .. })) => {
            let key = key.to_string();
            !matches!(key.as_str(), "rsver" | "rssdkver")
        }
        _ => true,
    })
    .collect::<Result<Vec<_>, _>>()
    .unwrap()
}

fn manifest_path_arg(path: &str) -> String {
    let arg = format!("--manifest-path={path}");
    escape(std::borrow::Cow::Owned(arg)).into_owned()
}

fn add_path() -> String {
    PathBuf::new()
        .join("contracts")
        .join("add")
        .join("Cargo.toml")
        .to_string_lossy()
        .to_string()
}

fn call_path() -> String {
    PathBuf::new()
        .join("contracts")
        .join("call")
        .join("Cargo.toml")
        .to_string_lossy()
        .to_string()
}

fn add2_path() -> String {
    PathBuf::new()
        .join("contracts")
        .join("add")
        .join("add2")
        .join("Cargo.toml")
        .to_string_lossy()
        .to_string()
}

fn parent_path() -> String {
    PathBuf::new()
        .join("..")
        .join("Cargo.toml")
        .to_string_lossy()
        .to_string()
}

fn with_flags(expected: &str) -> String {
    const ENV_VAR: &str = "SOROBAN_SDK_BUILD_SYSTEM_SUPPORTS_SPEC_SHAKING_V2=1";

    let cargo_home = home::cargo_home().unwrap();
    let registry_prefix = cargo_home.join("registry").join("src");
    let registry_prefix = registry_prefix.display().to_string();
    #[cfg(windows)]
    let registry_prefix = registry_prefix.replace('\\', "/");

    let vec: Vec<_> = if env::var("RUSTFLAGS").is_ok() {
        expected
            .split('\n')
            .map(|x| format!("{ENV_VAR} {x}"))
            .collect()
    } else {
        expected
            .split('\n')
            .map(|x| {
                let rustflags_value = format!("--remap-path-prefix={registry_prefix}=");
                let escaped_value = escape(std::borrow::Cow::Borrowed(&rustflags_value));
                format!("CARGO_BUILD_RUSTFLAGS={escaped_value} {ENV_VAR} {x}")
            })
            .collect()
    };

    format!(
        "\
{}
",
        vec.join("\n")
    )
}

// Test that bins don't contain absolute paths to the local crate registry.
//
// See make_rustflags_to_remap_absolute_paths
#[test]
#[ignore = "TODO https://github.com/stellar/stellar-cli/issues/1867"]
fn remap_absolute_paths() {
    #[derive(Eq, PartialEq, Copy, Clone)]
    enum Remap {
        Yes,
        No,
    }

    fn run(contract_name: &str, manifest_path: &std::path::Path, remap: Remap) -> bool {
        let sandbox_remap = TestEnv::default();
        let mut cmd = sandbox_remap.new_assert_cmd("contract");

        if remap == Remap::No {
            // This will prevent stellar-cli from setting CARGO_BUILD_RUSTFLAGS,
            // and removing absolute paths.
            // See docs for `make_rustflags_to_remap_absolute_paths`.
            cmd.env("RUSTFLAGS", "");
        }

        cmd.current_dir(manifest_path)
            .arg("build")
            .assert()
            .success();

        let wasm_path = manifest_path
            .join("target/wasm32v1-none/release")
            .join(format!("{contract_name}.wasm"));

        let cargo_home = home::cargo_home().unwrap();
        let registry_prefix = format!("{}/registry/src/", cargo_home.display());

        let wasm_buf = std::fs::read(wasm_path).unwrap();
        let wasm_str = String::from_utf8_lossy(&wasm_buf);

        wasm_str.contains(&registry_prefix)
    }

    let cargo_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_path = cargo_dir.join("tests/fixtures/eth_abi/");

    // The eth_abi example is known to exhibit this problem.
    // Compile it both with and without path remapping to verify.
    let remap_has_abs_paths = run("soroban_eth_abi", &fixture_path, Remap::Yes);
    let noremap_has_abs_paths = run("soroban_eth_abi", &fixture_path, Remap::No);

    assert!(!remap_has_abs_paths);
    assert!(noremap_has_abs_paths);
}

#[test]
fn build_no_error_for_workspace() {
    let sandbox = TestEnv::default();
    let cargo_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_path = cargo_dir.join("tests/fixtures/workspace");
    let temp = TempDir::new().unwrap();
    let dir_path = temp.path();
    fs_extra::dir::copy(fixture_path, dir_path, &CopyOptions::new()).unwrap();
    let dir_path = dir_path.join("workspace");

    // By default, workspace TOML has overflow-checks = true

    sandbox
        .new_assert_cmd("contract")
        .current_dir(&dir_path)
        .arg("build")
        .assert()
        .success();
}

#[test]
fn build_no_error_for_package() {
    let sandbox = TestEnv::default();
    let cargo_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_path = cargo_dir.join("tests/fixtures/workspace/contracts/add/add2");
    let temp = TempDir::new().unwrap();
    let dir_path = temp.path();
    fs_extra::dir::copy(fixture_path, dir_path, &CopyOptions::new()).unwrap();
    let dir_path = dir_path.join("add2");

    // By default, this TOML does not specify overflow-checks, add it
    let cargo_toml_path = dir_path.join("Cargo.toml");
    let cargo_toml_path_content = std::fs::read_to_string(&cargo_toml_path).unwrap();
    let modified_cargo_toml_content =
        format!("{cargo_toml_path_content}\n[profile.release]\noverflow-checks = true\n");
    std::fs::write(&cargo_toml_path, modified_cargo_toml_content).unwrap();

    sandbox
        .new_assert_cmd("contract")
        .current_dir(&dir_path)
        .arg("build")
        .assert()
        .success();
}

#[test]
fn build_errors_when_overflow_checks_false() {
    let sandbox = TestEnv::default();
    let cargo_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_path = cargo_dir.join("tests/fixtures/workspace");
    let temp = TempDir::new().unwrap();
    let dir_path = temp.path();
    fs_extra::dir::copy(fixture_path, dir_path, &CopyOptions::new()).unwrap();
    let dir_path = dir_path.join("workspace");

    // Replace overflow-checks = true with false in workspace Cargo.toml
    let cargo_toml_path = dir_path.join("Cargo.toml");
    let cargo_toml_content = std::fs::read_to_string(&cargo_toml_path).unwrap();
    let modified_content =
        cargo_toml_content.replace("overflow-checks = true", "overflow-checks = false");
    std::fs::write(&cargo_toml_path, modified_content).unwrap();

    sandbox
        .new_assert_cmd("contract")
        .current_dir(&dir_path)
        .arg("build")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "`overflow-checks` is not enabled for profile `release`",
        ));
}

#[test]
fn build_errors_when_overflow_checks_missing() {
    let sandbox = TestEnv::default();
    let cargo_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_path = cargo_dir.join("tests/fixtures/workspace");
    let temp = TempDir::new().unwrap();
    let dir_path = temp.path();
    fs_extra::dir::copy(fixture_path, dir_path, &CopyOptions::new()).unwrap();
    let dir_path = dir_path.join("workspace");

    // Remove overflow-checks line from workspace Cargo.toml
    let cargo_toml_path = dir_path.join("Cargo.toml");
    let cargo_toml_content = std::fs::read_to_string(&cargo_toml_path).unwrap();
    let modified_content = cargo_toml_content
        .replace("overflow-checks = true\r\n", "")
        .replace("overflow-checks = true\n", "");
    std::fs::write(&cargo_toml_path, modified_content).unwrap();

    sandbox
        .new_assert_cmd("contract")
        .current_dir(&dir_path)
        .arg("build")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "`overflow-checks` is not enabled for profile `release`",
        ));
}

#[test]
fn build_errors_when_package_overflow_checks_missing() {
    let sandbox = TestEnv::default();
    let cargo_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_path = cargo_dir.join("tests/fixtures/workspace/contracts/add/add2");
    let temp = TempDir::new().unwrap();
    let dir_path = temp.path();
    fs_extra::dir::copy(fixture_path, dir_path, &CopyOptions::new()).unwrap();
    let dir_path = dir_path.join("add2");

    // By default, this TOML does not specify overflow-checks

    sandbox
        .new_assert_cmd("contract")
        .current_dir(&dir_path)
        .arg("build")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "`overflow-checks` is not enabled for profile `release`",
        ));
}

#[test]
fn build_errors_when_overflow_check_only_applied_to_members() {
    let sandbox = TestEnv::default();
    let cargo_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_path = cargo_dir.join("tests/fixtures/workspace");
    let temp = TempDir::new().unwrap();
    let dir_path = temp.path();
    fs_extra::dir::copy(fixture_path, dir_path, &CopyOptions::new()).unwrap();
    let dir_path = dir_path.join("workspace");

    // Remove overflow-checks line from workspace Cargo.toml
    let cargo_toml_path = dir_path.join("Cargo.toml");
    let cargo_toml_content = std::fs::read_to_string(&cargo_toml_path).unwrap();
    let modified_content = cargo_toml_content
        .replace("overflow-checks = true\r\n", "")
        .replace("overflow-checks = true\n", "");
    std::fs::write(&cargo_toml_path, modified_content).unwrap();

    // Add overflow-checks = true to "add" member
    let member_cargo_toml_path = dir_path.join("contracts").join("add").join("Cargo.toml");
    let member_cargo_toml_content = std::fs::read_to_string(&member_cargo_toml_path).unwrap();
    let modified_member_content =
        format!("{member_cargo_toml_content}\n[profile.release]\noverflow-checks = true\n");
    std::fs::write(&member_cargo_toml_path, modified_member_content).unwrap();

    // Add overflow-checks = true to "add2" member
    let member_2_cargo_toml_path = dir_path
        .join("contracts")
        .join("add")
        .join("add2")
        .join("Cargo.toml");
    let member_2_cargo_toml_content = std::fs::read_to_string(&member_2_cargo_toml_path).unwrap();
    let modified_member_2_content =
        format!("{member_2_cargo_toml_content}\n[profile.release]\noverflow-checks = true\n");
    std::fs::write(&member_2_cargo_toml_path, modified_member_2_content).unwrap();

    sandbox
        .new_assert_cmd("contract")
        .current_dir(&dir_path)
        .arg("build")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "`overflow-checks` is not enabled for profile `release`",
        ));
}

#[test]
fn build_no_error_with_print_commands_only() {
    let sandbox = TestEnv::default();
    let temp = TempDir::new().unwrap();
    let dir_path = temp.path();

    // Create a workspace without overflow-checks
    std::fs::write(
        dir_path.join("Cargo.toml"),
        r#"
[workspace]
resolver = "2"
members = ["contract"]

[profile.release]
opt-level = "z"
"#,
    )
    .unwrap();

    std::fs::create_dir_all(dir_path.join("contract/src")).unwrap();
    std::fs::write(
        dir_path.join("contract/Cargo.toml"),
        r#"
[package]
name = "test-contract"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]
"#,
    )
    .unwrap();
    std::fs::write(dir_path.join("contract/src/lib.rs"), "").unwrap();

    // With --print-commands-only, no warning should appear
    sandbox
        .new_assert_cmd("contract")
        .current_dir(dir_path)
        .arg("build")
        .arg("--print-commands-only")
        .assert()
        .success()
        .stderr(predicate::str::contains("overflow-checks").not());
}

#[test]
fn build_always_injects_cli_version() {
    let sandbox = TestEnv::default();
    let outdir = sandbox.dir().join("out");
    let cargo_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_path = cargo_dir.join("tests/fixtures/workspace");
    let temp = TempDir::new().unwrap();
    let dir_path = temp.path();
    fs_extra::dir::copy(fixture_path, dir_path, &CopyOptions::new()).unwrap();
    let dir_path = dir_path.join("workspace").join("contracts").join("add");

    // Build contract without any metadata args
    sandbox
        .new_assert_cmd("contract")
        .current_dir(&dir_path)
        .arg("build")
        .arg("--out-dir")
        .arg(&outdir)
        .assert()
        .success();

    let entries = get_entries(&dir_path, &outdir);

    // Verify that CLI version is present
    let cli_version_entry = entries
        .iter()
        .find(|entry| matches!(entry, ScMetaEntry::ScMetaV0(ScMetaV0 { key, .. }) if key.to_string() == "cliver"))
        .expect("CLI version metadata entry should be present");

    let ScMetaEntry::ScMetaV0(ScMetaV0 { val, .. }) = cli_version_entry;
    let version_string = val.to_string();
    assert!(
        version_string.contains('#'),
        "CLI version should be in format 'version#git'"
    );
    assert!(
        !version_string.is_empty(),
        "CLI version should not be empty"
    );
}

const ZERO_DIGEST: &str =
    "docker.io/stellar/stellar-cli@sha256:0000000000000000000000000000000000000000000000000000000000000000";

// Convenience: drive a git command in a fixture directory.
fn git_in(dir: &Path, args: &[&str]) {
    std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@example.com")
        .status()
        .unwrap();
}

// Init a tempdir copy of the workspace fixture and return the workspace path.
fn fresh_workspace() -> (TempDir, PathBuf) {
    let cargo_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_path = cargo_dir.join("tests/fixtures/workspace");
    let temp = TempDir::new().unwrap();
    fs_extra::dir::copy(&fixture_path, temp.path(), &CopyOptions::new()).unwrap();
    let workspace = temp.path().join("workspace");
    (temp, workspace)
}

// `--verifiable` cannot accept reserved `--meta` keys that the cli writes itself.
#[test]
fn verifiable_meta_conflict_errors() {
    let sandbox = TestEnv::default();
    let cargo_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_path = cargo_dir.join("tests/fixtures/workspace/contracts/add");

    sandbox
        .new_assert_cmd("contract")
        .current_dir(fixture_path)
        .arg("build")
        .arg("--verifiable")
        .arg("--image")
        .arg(ZERO_DIGEST)
        .arg("--source-sha256")
        .arg("a".repeat(64))
        .arg("--meta")
        .arg("bldimg=not-allowed")
        .assert()
        .failure()
        .stderr(predicate::str::contains("reserved key: bldimg"));
}

// `--image` is validated against the SEP-58 bldimg regex; tag-only refs fail.
#[test]
fn verifiable_image_must_be_digest_pinned() {
    let sandbox = TestEnv::default();
    let cargo_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_path = cargo_dir.join("tests/fixtures/workspace/contracts/add");

    sandbox
        .new_assert_cmd("contract")
        .current_dir(fixture_path)
        .arg("build")
        .arg("--verifiable")
        .arg("--image")
        .arg("docker.io/stellar/stellar-cli:latest")
        .arg("--source-sha256")
        .arg("a".repeat(64))
        .assert()
        .failure()
        .stderr(predicate::str::contains("bldimg format"));
}

// SEP-58 bldimg requires an explicit registry host (e.g. `docker.io/...`).
// Implicit Docker-Hub-style short refs are rejected.
#[test]
fn verifiable_image_requires_explicit_registry_host() {
    let sandbox = TestEnv::default();
    let cargo_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_path = cargo_dir.join("tests/fixtures/workspace/contracts/add");

    let short_ref = format!("stellar/stellar-cli@sha256:{}", "0".repeat(64));

    sandbox
        .new_assert_cmd("contract")
        .current_dir(fixture_path)
        .arg("build")
        .arg("--verifiable")
        .arg("--image")
        .arg(short_ref)
        .arg("--source-sha256")
        .arg("a".repeat(64))
        .assert()
        .failure()
        .stderr(predicate::str::contains("bldimg format"));
}

// `--verifiable` always generates the source archive (and computes
// source_sha256) before the docker stage, so the "Wrote source archive" line
// appears even though the build then fails to reach a real image.
#[test]
fn verifiable_always_writes_source_archive() {
    let sandbox = TestEnv::default();
    let (_temp, workspace) = fresh_workspace();
    git_in(&workspace, &["init", "-q", "-b", "main"]);
    git_in(&workspace, &["add", "-A"]);
    git_in(&workspace, &["commit", "-q", "-m", "init"]);

    sandbox
        .new_assert_cmd("contract")
        .current_dir(workspace.join("contracts").join("add"))
        .arg("build")
        .arg("--verifiable")
        .arg("--image")
        .arg(ZERO_DIGEST)
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("Wrote source archive")
                .and(predicate::str::contains("source_sha256")),
        );
}

// `contract archive --out` writes the gzipped tarball and prints its
// source_sha256.
#[test]
fn contract_archive_writes_out() {
    let sandbox = TestEnv::default();
    let (temp, workspace) = fresh_workspace();
    git_in(&workspace, &["init", "-q", "-b", "main"]);
    git_in(&workspace, &["add", "-A"]);
    git_in(&workspace, &["commit", "-q", "-m", "init"]);

    let out = temp.path().join("src.tar.gz");

    sandbox
        .new_assert_cmd("contract")
        .current_dir(&workspace)
        .arg("archive")
        .arg("--out-file")
        .arg(&out)
        .assert()
        .success()
        .stderr(
            predicate::str::contains("Wrote source archive")
                .and(predicate::str::contains("source_sha256")),
        );

    assert!(out.exists(), "the archive should be written to --out");
    assert!(
        std::fs::metadata(&out).unwrap().len() > 0,
        "the archive should not be empty"
    );
}

// `contract archive --dry-run` lists the archived entries and the
// source_sha256 without writing any file.
#[test]
fn contract_archive_dry_run_lists_entries() {
    let sandbox = TestEnv::default();
    let (temp, workspace) = fresh_workspace();
    git_in(&workspace, &["init", "-q", "-b", "main"]);
    git_in(&workspace, &["add", "-A"]);
    git_in(&workspace, &["commit", "-q", "-m", "init"]);

    let out = temp.path().join("should-not-exist.tar.gz");

    sandbox
        .new_assert_cmd("contract")
        .current_dir(&workspace)
        .arg("archive")
        .arg("--dry-run")
        .assert()
        .success()
        .stdout(predicate::str::contains("source/Cargo.toml"))
        .stderr(predicate::str::contains("source_sha256"));

    assert!(!out.exists(), "--dry-run must not write an archive");
}

// `--out-file` must name a gzipped tarball (.tar.gz / .tgz).
#[test]
fn contract_archive_rejects_bad_out_file_extension() {
    let sandbox = TestEnv::default();
    let (temp, workspace) = fresh_workspace();
    git_in(&workspace, &["init", "-q", "-b", "main"]);
    git_in(&workspace, &["add", "-A"]);
    git_in(&workspace, &["commit", "-q", "-m", "init"]);

    let out = temp.path().join("src.zip");

    sandbox
        .new_assert_cmd("contract")
        .current_dir(&workspace)
        .arg("archive")
        .arg("--out-file")
        .arg(&out)
        .assert()
        .failure()
        .stderr(predicate::str::contains(".tar.gz or .tgz"));

    assert!(
        !out.exists(),
        "no archive should be written on a bad extension"
    );
}

// `--out-file` is required unless `--dry-run` is passed.
#[test]
fn contract_archive_requires_out_file_without_dry_run() {
    let sandbox = TestEnv::default();
    let (_temp, workspace) = fresh_workspace();

    sandbox
        .new_assert_cmd("contract")
        .current_dir(&workspace)
        .arg("archive")
        .assert()
        .failure()
        .stderr(predicate::str::contains("--out-file"));
}

// A dirty git tree is a hard fail for `contract archive` too, matching
// `--verifiable`: the source_sha256 must describe a committed state.
#[test]
fn contract_archive_dirty_tree_errors() {
    let sandbox = TestEnv::default();
    let (temp, workspace) = fresh_workspace();
    git_in(&workspace, &["init", "-q", "-b", "main"]);
    git_in(&workspace, &["add", "-A"]);
    git_in(&workspace, &["commit", "-q", "-m", "init"]);
    // Dirty the tree after committing so status is non-empty.
    std::fs::write(workspace.join("dirty.txt"), b"uncommitted").unwrap();

    let out = temp.path().join("src.tar.gz");

    sandbox
        .new_assert_cmd("contract")
        .current_dir(&workspace)
        .arg("archive")
        .arg("--out-file")
        .arg(&out)
        .assert()
        .failure()
        .stderr(predicate::str::contains("dirty"));

    assert!(
        !out.exists(),
        "no archive should be written for a dirty tree"
    );
}

// `--source-sha256` value must match the 64-hex regex.
#[test]
fn verifiable_source_sha256_format_errors() {
    let sandbox = TestEnv::default();
    let cargo_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_path = cargo_dir.join("tests/fixtures/workspace/contracts/add");

    sandbox
        .new_assert_cmd("contract")
        .current_dir(fixture_path)
        .arg("build")
        .arg("--verifiable")
        .arg("--image")
        .arg(ZERO_DIGEST)
        .arg("--source-sha256")
        .arg("not-a-sha")
        .assert()
        .failure()
        .stderr(predicate::str::contains("source_sha256 format"));
}

// `--source-uri` value must be a URI with a scheme.
#[test]
fn verifiable_source_uri_format_errors() {
    let sandbox = TestEnv::default();
    let cargo_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_path = cargo_dir.join("tests/fixtures/workspace/contracts/add");

    sandbox
        .new_assert_cmd("contract")
        .current_dir(fixture_path)
        .arg("build")
        .arg("--verifiable")
        .arg("--image")
        .arg(ZERO_DIGEST)
        .arg("--source-sha256")
        .arg("a".repeat(64))
        .arg("--source-uri")
        .arg("not a uri")
        .assert()
        .failure()
        .stderr(predicate::str::contains("source_uri format"));
}

// A dirty git tree is a hard fail under `--verifiable` (the recorded
// source_sha256 would not describe the bytes built).
#[test]
fn verifiable_dirty_tree_errors() {
    let sandbox = TestEnv::default();
    let (_temp, workspace) = fresh_workspace();
    git_in(&workspace, &["init", "-q", "-b", "main"]);
    git_in(&workspace, &["add", "-A"]);
    git_in(&workspace, &["commit", "-q", "-m", "init"]);
    // Dirty the tree after committing so status is non-empty.
    std::fs::write(workspace.join("dirty.txt"), b"uncommitted").unwrap();

    sandbox
        .new_assert_cmd("contract")
        .current_dir(workspace.join("contracts").join("add"))
        .arg("build")
        .arg("--verifiable")
        .arg("--image")
        .arg(ZERO_DIGEST)
        .arg("--source-sha256")
        .arg("a".repeat(64))
        .assert()
        .failure()
        .stderr(predicate::str::contains("dirty").or(predicate::str::contains("clean tree")));
}
