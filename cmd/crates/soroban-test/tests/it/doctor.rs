/*
`doctor` reports on things it discovers outside the process: which Stellar CLI
executables are on `PATH`, and which CLI last checked for a new release and
wrote the shared version cache. Both inputs are injectable -- `PATH` like in
`plugin.rs`, and the cache via `STELLAR_DATA_HOME` -- so the reporting can be
exercised end to end.

Unix only: the fake CLIs are shell scripts that need an execute bit.
*/

use std::ffi::OsStr;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;
use soroban_cli::commands::version::pkg;
use soroban_test::TestEnv;

/// A stand-in CLI on `PATH` that answers version queries and nothing else.
///
/// `supports_only_version` distinguishes a current CLI from one old enough to
/// reject `version --only-version` -- the releases that cause the version
/// confusion these checks exist to surface only answer `--version`.
fn write_fake_cli(dir: &Path, name: &str, version: &str, supports_only_version: bool) {
    let only_version = if supports_only_version {
        format!("echo \"{version}\"")
    } else {
        "echo \"error: unexpected argument '--only-version' found\" >&2; exit 2".to_string()
    };

    let script = format!(
        "#!/bin/sh\n\
         case \"$*\" in\n\
           \"version --only-version\") {only_version} ;;\n\
           \"--version\") echo \"stellar {version} (0000000000000000000000000000000000000000)\" ;;\n\
           *) echo \"unexpected args: $*\" >&2; exit 1 ;;\n\
         esac\n"
    );

    let path = dir.join(name);
    fs::write(&path, script).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
}

/// A CLI on `PATH` that cannot be asked for its version -- an install too
/// broken to answer either version query.
fn write_unrunnable_cli(dir: &Path, name: &str) {
    let path = dir.join(name);
    fs::write(&path, "#!/bin/sh\nexit 1\n").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
}

/// A directory under the sandbox, in the same canonicalized form `doctor`
/// reports paths in.
///
/// `find_installs` canonicalizes every executable it discovers, so an expected
/// path built from an uncanonicalized sandbox never matches: on macOS the
/// temporary directory sits under `/var/folders`, a symlink to `/private/var`,
/// and the two spellings compare unequal as strings.
fn empty_dir(sandbox: &TestEnv, name: &str) -> PathBuf {
    let dir = sandbox.dir().join(name);
    fs::create_dir_all(&dir).unwrap();
    dir.canonicalize().unwrap_or(dir)
}

/// The path `doctor` sees as its own executable, in the canonicalized form it
/// compares cache entries against.
fn running_binary() -> String {
    let path = assert_cmd::cargo::cargo_bin("stellar");
    let path = path.canonicalize().unwrap_or(path);
    path.to_string_lossy().into_owned()
}

/// Seed the shared version cache with a given writer, and return the data home
/// that holds it.
///
/// `latest_check_time` is irrelevant to what is asserted here: `doctor` calls
/// `has_available_upgrade` with caching off, so it always attempts a refresh.
/// The seeded writer is still what gets reported, because `doctor` reads it
/// before that refresh can overwrite it.
fn seed_cache_writer(sandbox: &TestEnv, writer: serde_json::Value) -> PathBuf {
    let data_home = empty_dir(sandbox, "cache-data-home");

    let cache = serde_json::json!({
        "latest_check_time": "2026-08-04T10:00:00Z",
        "max_stable_version": "27.1.0",
        "max_version": "27.1.0",
        "last_checked_by": writer,
    });

    fs::write(
        data_home.join("upgrade_check.json"),
        serde_json::to_string(&cache).unwrap(),
    )
    .unwrap();

    data_home
}

/// `doctor` with `PATH` and the version cache pointed at test-controlled state.
///
/// `path` is a whole `PATH` value, not a single directory: a multi-entry one is
/// several paths joined by `:` and is no longer a path itself.
fn doctor(sandbox: &TestEnv, path: impl AsRef<OsStr>, data_home: &Path) -> Command {
    let mut cmd = sandbox.new_assert_cmd("doctor");
    cmd.env("PATH", path).env("STELLAR_DATA_HOME", data_home);
    cmd
}

#[test]
fn reports_the_running_executable_and_a_lone_install() {
    let sandbox = TestEnv::default();
    let bin_dir = empty_dir(&sandbox, "one-install");
    let data_home = empty_dir(&sandbox, "data-home");
    write_fake_cli(&bin_dir, "stellar", "27.1.0", true);

    doctor(&sandbox, &bin_dir, &data_home)
        .assert()
        .success()
        .stderr(contains(format!(
            "Running executable: {}",
            running_binary()
        )))
        .stderr(contains("Only one Stellar CLI found on PATH"))
        // The lone install is listed with its version too: "Running executable"
        // is not necessarily the entry `PATH` resolves by name, and carries no
        // version of its own.
        .stderr(contains(format!(
            "- {} (27.1.0)",
            bin_dir.join("stellar").to_string_lossy()
        )));
}

#[test]
fn reports_when_no_cli_is_on_path() {
    let sandbox = TestEnv::default();
    let bin_dir = empty_dir(&sandbox, "no-installs");
    let data_home = empty_dir(&sandbox, "data-home");

    doctor(&sandbox, &bin_dir, &data_home)
        .assert()
        .success()
        .stderr(contains("No Stellar CLI found on PATH"))
        // Nothing was found, so claiming a single install would be a lie.
        .stderr(contains("Only one Stellar CLI").not());
}

#[test]
fn does_not_warn_when_both_binary_names_report_the_same_version() {
    let sandbox = TestEnv::default();
    let bin_dir = empty_dir(&sandbox, "two-installs-agreeing");
    let data_home = empty_dir(&sandbox, "data-home");
    // What an ordinary install looks like: one crate, two binary names.
    write_fake_cli(&bin_dir, "stellar", "27.1.0", true);
    write_fake_cli(&bin_dir, "soroban", "27.1.0", true);

    doctor(&sandbox, &bin_dir, &data_home)
        .assert()
        .success()
        .stderr(contains(
            "2 Stellar CLI executables on PATH, all reporting 27.1.0",
        ))
        .stderr(contains("different versions").not());
}

#[test]
fn warns_when_installs_report_different_versions() {
    let sandbox = TestEnv::default();
    let bin_dir = empty_dir(&sandbox, "two-installs-disagreeing");
    let data_home = empty_dir(&sandbox, "data-home");
    write_fake_cli(&bin_dir, "stellar", "27.1.0", true);
    // Old enough to only answer `--version`, which is the fallback path.
    write_fake_cli(&bin_dir, "soroban", "22.8.0", false);

    doctor(&sandbox, &bin_dir, &data_home)
        .assert()
        .success()
        .stderr(contains(
            "Found 2 Stellar CLI executables on PATH reporting different versions",
        ))
        .stderr(contains(format!(
            "- {} (27.1.0)",
            bin_dir.join("stellar").to_string_lossy()
        )))
        .stderr(contains(format!(
            "- {} (22.8.0)",
            bin_dir.join("soroban").to_string_lossy()
        )));
}

#[test]
fn does_not_blame_differing_versions_when_a_version_could_not_be_read() {
    let sandbox = TestEnv::default();
    let bin_dir = empty_dir(&sandbox, "two-installs-unreadable");
    let data_home = empty_dir(&sandbox, "data-home");
    // Neither answers, so nothing was observed to disagree -- the report must
    // say the versions are unknown, not that they differ.
    write_unrunnable_cli(&bin_dir, "stellar");
    write_unrunnable_cli(&bin_dir, "soroban");

    doctor(&sandbox, &bin_dir, &data_home)
        .assert()
        .success()
        .stderr(contains(
            "Found 2 Stellar CLI executables on PATH; none of them reported a version",
        ))
        .stderr(contains("different versions").not())
        .stderr(contains(format!(
            "- {} (unknown version)",
            bin_dir.join("stellar").to_string_lossy()
        )));
}

#[test]
fn reports_a_disagreement_even_when_another_executable_is_unreadable() {
    let sandbox = TestEnv::default();
    let bin_dir = empty_dir(&sandbox, "disagreeing-and-unreadable");
    let second_dir = empty_dir(&sandbox, "disagreeing-and-unreadable-2");
    let data_home = empty_dir(&sandbox, "data-home");
    write_fake_cli(&bin_dir, "stellar", "27.1.0", true);
    write_fake_cli(&bin_dir, "soroban", "22.8.0", false);
    write_unrunnable_cli(&second_dir, "stellar");

    let path = format!(
        "{}:{}",
        bin_dir.to_string_lossy(),
        second_dir.to_string_lossy()
    );

    doctor(&sandbox, &path, &data_home)
        .assert()
        .success()
        // An observed disagreement is a fact; a failed probe alongside it does
        // not soften it. It does not join it either: only two executables were
        // heard from, so only two can be said to disagree.
        .stderr(contains(
            "Found 3 Stellar CLI executables on PATH; the 2 that reported a version do not \
             agree (1 could not be asked)",
        ));
}

#[test]
fn reports_the_agreement_among_the_executables_that_answered() {
    let sandbox = TestEnv::default();
    let bin_dir = empty_dir(&sandbox, "agreeing-and-unreadable");
    let second_dir = empty_dir(&sandbox, "agreeing-and-unreadable-2");
    let data_home = empty_dir(&sandbox, "data-home");
    write_fake_cli(&bin_dir, "stellar", "27.1.0", true);
    write_fake_cli(&bin_dir, "soroban", "27.1.0", true);
    write_unrunnable_cli(&second_dir, "stellar");

    let path = format!(
        "{}:{}",
        bin_dir.to_string_lossy(),
        second_dir.to_string_lossy()
    );

    doctor(&sandbox, &path, &data_home)
        .assert()
        .success()
        // The two that answered agree, and that is the most useful thing known
        // here -- the executable that could not be asked leaves it standing
        // rather than wiping it out.
        .stderr(contains(
            "every one that answered reports 27.1.0, but 1 could not be asked",
        ))
        .stderr(contains("different versions").not());
}

#[test]
fn confirms_the_cache_writer_when_it_is_this_cli() {
    let sandbox = TestEnv::default();
    let bin_dir = empty_dir(&sandbox, "bin");
    let data_home = seed_cache_writer(
        &sandbox,
        serde_json::json!({ "version": pkg(), "executable": running_binary() }),
    );

    doctor(&sandbox, &bin_dir, &data_home)
        .assert()
        .success()
        .stderr(contains(format!(
            "Version cache last checked by: {} ({})",
            pkg(),
            running_binary()
        )));
}

#[test]
fn does_not_warn_when_the_same_install_wrote_the_cache_at_an_older_version() {
    let sandbox = TestEnv::default();
    let bin_dir = empty_dir(&sandbox, "bin");
    // An in-place upgrade: same path, earlier version recorded against it.
    let data_home = seed_cache_writer(
        &sandbox,
        serde_json::json!({ "version": "26.1.0", "executable": running_binary() }),
    );

    doctor(&sandbox, &bin_dir, &data_home)
        .assert()
        .success()
        .stderr(contains(format!(
            "Version cache was last checked by this install running 26.1.0; it is now {}",
            pkg()
        )))
        .stderr(contains("a different Stellar CLI").not());
}

#[test]
fn warns_when_a_different_install_wrote_the_cache() {
    let sandbox = TestEnv::default();
    let bin_dir = empty_dir(&sandbox, "bin");
    let data_home = seed_cache_writer(
        &sandbox,
        serde_json::json!({ "version": "22.8.0", "executable": "/opt/elsewhere/bin/soroban" }),
    );

    doctor(&sandbox, &bin_dir, &data_home)
        .assert()
        .success()
        .stderr(contains(
            "Version cache was last checked by a different Stellar CLI: \
             22.8.0 (/opt/elsewhere/bin/soroban)",
        ))
        .stderr(contains(format!("this one is {} (", pkg())));
}

#[test]
fn reports_an_unknown_cache_writer_without_claiming_a_mismatch() {
    let sandbox = TestEnv::default();
    let bin_dir = empty_dir(&sandbox, "bin");
    // A cache written before the writer was recorded.
    let data_home = seed_cache_writer(&sandbox, serde_json::Value::Null);

    doctor(&sandbox, &bin_dir, &data_home)
        .assert()
        .success()
        .stderr(contains(
            "Version cache was last checked by an unknown Stellar CLI",
        ))
        .stderr(contains("a different Stellar CLI").not());
}
