use assert_fs::TempDir;
use fs_extra::dir::CopyOptions;
use predicates::prelude::predicate;
use soroban_cli::xdr::{Limited, Limits, ReadXdr, ScMetaEntry, ScMetaV0};
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
        "cargo rustc --manifest-path={} --crate-type=cdylib --target=wasm32v1-none --release
cargo rustc --manifest-path={} --crate-type=cdylib --target=wasm32v1-none --release
cargo rustc --manifest-path={} --crate-type=cdylib --target=wasm32v1-none --release",
        add_path(),
        call_path(),
        add2_path()
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
        "cargo rustc --manifest-path={} --crate-type=cdylib --target=wasm32v1-none --release",
        add_path()
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
        "cargo rustc --manifest-path={} --crate-type=cdylib --target=wasm32v1-none --release",
        parent_path()
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
        "cargo rustc --manifest-path={} --crate-type=cdylib --target=wasm32v1-none --release",
        add_path()
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
    let fixture_path = cargo_dir.join("tests/fixtures/workspace/contracts/add");
    let temp = TempDir::new().unwrap();
    let dir_path = temp.path();
    fs_extra::dir::copy(fixture_path, dir_path, &CopyOptions::new()).unwrap();
    let dir_path = dir_path.join("add");

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

#[test]
fn build_with_metadata_diff_dir() {
    let sandbox = TestEnv::default();
    let outdir1 = sandbox.dir().join("out-1");
    let outdir2 = sandbox.dir().join("out-2");
    let cargo_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_path = cargo_dir.join("tests/fixtures/workspace/contracts/add");
    let temp = TempDir::new().unwrap();
    let dir_path = temp.path();
    fs_extra::dir::copy(fixture_path, dir_path, &CopyOptions::new()).unwrap();
    let dir_path = dir_path.join("add");

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

    let entries_dir2 = get_entries(&dir_path, &outdir2);
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

    assert_eq!(entries_dir1, expected_entries_dir1);
    assert_eq!(entries_dir2, expected_entries_dir2);
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
    let cargo_home = home::cargo_home().unwrap();
    let registry_prefix = cargo_home.join("registry").join("src");
    let registry_prefix = registry_prefix.display();

    let vec: Vec<_> = if env::var("RUSTFLAGS").is_ok() {
        expected.split('\n').map(ToString::to_string).collect()
    } else {
        expected
            .split('\n')
            .map(|x| format!("CARGO_BUILD_RUSTFLAGS=--remap-path-prefix={registry_prefix}= {x}",))
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
        let registry_prefix = format!("{}/registry/src/", &cargo_home.display());

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
