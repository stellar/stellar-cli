use predicates::prelude::predicate;
use soroban_cli::xdr::{Limited, Limits, ReadXdr, ScMetaEntry, ScMetaV0};
use soroban_spec_tools::contract::Spec;
use soroban_test::TestEnv;
use std::env;
use std::io::Cursor;

#[test]
fn build_all() {
    let sandbox = TestEnv::default();
    let cargo_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_path = cargo_dir.join("tests/fixtures/workspace/");
    sandbox
        .new_assert_cmd("contract")
        .current_dir(fixture_path)
        .arg("build")
        .arg("--print-commands-only")
        .assert()
        .success()
        .stdout(predicate::eq(with_flags("cargo rustc --manifest-path=contracts/add/Cargo.toml --crate-type=cdylib --target=wasm32-unknown-unknown --release
cargo rustc --manifest-path=contracts/call/Cargo.toml --crate-type=cdylib --target=wasm32-unknown-unknown --release
cargo rustc --manifest-path=contracts/add/add2/Cargo.toml --crate-type=cdylib --target=wasm32-unknown-unknown --release")));
}

#[test]
fn build_package_by_name() {
    let sandbox = TestEnv::default();
    let cargo_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_path = cargo_dir.join("tests/fixtures/workspace/");
    sandbox
        .new_assert_cmd("contract")
        .current_dir(fixture_path)
        .arg("build")
        .arg("--print-commands-only")
        .arg("--package=add")
        .assert()
        .success()
        .stdout(predicate::eq(with_flags("cargo rustc --manifest-path=contracts/add/Cargo.toml --crate-type=cdylib --target=wasm32-unknown-unknown --release")));
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
            with_flags("cargo rustc --manifest-path=Cargo.toml --crate-type=cdylib --target=wasm32-unknown-unknown --release"),
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

    sandbox
        .new_assert_cmd("contract")
        .current_dir(fixture_path)
        .arg("build")
        .arg("--print-commands-only")
        .assert()
        .success()
        .stdout(predicate::eq(with_flags(
            "cargo rustc --manifest-path=../Cargo.toml --crate-type=cdylib --target=wasm32-unknown-unknown --release"
        )));
}

#[test]
fn build_default_members() {
    let sandbox = TestEnv::default();
    let cargo_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_path = cargo_dir.join("tests/fixtures/workspace-with-default-members/");
    sandbox
        .new_assert_cmd("contract")
        .current_dir(fixture_path)
        .arg("build")
        .arg("--print-commands-only")
        .assert()
        .success()
        .stdout(predicate::eq(with_flags("cargo rustc --manifest-path=contracts/add/Cargo.toml --crate-type=cdylib --target=wasm32-unknown-unknown --release")));
}

#[test]
fn build_with_metadata() {
    let sandbox = TestEnv::default();
    let cargo_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_path = cargo_dir.join("tests/fixtures/workspace/contracts/add");
    let outdir = sandbox.dir().join("out");

    sandbox
        .new_assert_cmd("contract")
        .current_dir(&fixture_path)
        .arg("build")
        .arg("--meta")
        .arg("contract meta=added on build")
        .arg("--out-dir")
        .arg(&outdir)
        .assert()
        .success();

    sandbox
        .new_assert_cmd("contract")
        .current_dir(&fixture_path)
        .arg("build")
        .arg("--meta")
        .arg("meta_replaced=some_new_meta")
        .arg("--out-dir")
        .arg(&outdir)
        .assert()
        .success();

    // verify that the metadata added in the contract code via contractmetadata! macro is present
    // as well as the meta that is included on build
    let wasm_path = fixture_path.join(&outdir).join("add.wasm");
    let wasm = std::fs::read(wasm_path).unwrap();
    let spec = Spec::new(&wasm).unwrap();
    let meta = spec.meta_base64.unwrap();
    let entries = ScMetaEntry::read_xdr_base64_iter(&mut Limited::new(
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
    .unwrap();

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

    assert_eq!(entries, expected_entries);
}

fn with_flags(expected: &str) -> String {
    let cargo_home = home::cargo_home().unwrap();
    let cargo_home = format!("{}", cargo_home.display());
    let registry_prefix = format!("{cargo_home}/registry/src/");

    let vec: Vec<_> = if env::var("RUSTFLAGS").is_ok() {
        expected.split("\n").map(|x| x.to_string()).collect()
    } else {
        expected
            .split("\n")
            .map(|x| {
                format!(
                    "CARGO_BUILD_RUSTFLAGS=--remap-path-prefix={}= {}",
                    registry_prefix, x
                )
            })
            .collect()
    };

    return format!(
        "\
{}
",
        vec.join("\n")
    );
}
