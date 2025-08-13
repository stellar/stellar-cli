/*
This function calls the stellar executable via cargo and checks that the output
is correct. The PATH environment variable is set to include the target/bin
directory, so that the stellar executable can be found.
*/

use std::{ffi::OsString, path::PathBuf};

#[test]
fn soroban_hello() {
    // Add the target/bin directory to the iterator of paths
    let paths = get_paths();
    // Call soroban with the PATH variable set to include the target/bin directory
    assert_cmd::Command::cargo_bin("stellar")
        .unwrap_or_else(|_| assert_cmd::Command::new("soroban"))
        .arg("hello")
        .env("PATH", &paths)
        .assert()
        .stdout("Hello, world!\n");
}

#[test]
fn stellar_bye() {
    // Add the target/bin directory to the iterator of paths
    let paths = get_paths();
    // Call soroban with the PATH variable set to include the target/bin directory
    assert_cmd::Command::cargo_bin("stellar")
        .unwrap_or_else(|_| assert_cmd::Command::new("stellar"))
        .arg("bye")
        .env("PATH", &paths)
        .assert()
        .stdout("Bye, world!\n");
}

#[test]
fn list() {
    // Call `soroban --list` with the PATH variable set to include the target/bin directory
    assert_cmd::Command::cargo_bin("stellar")
        .unwrap_or_else(|_| assert_cmd::Command::new("stellar"))
        .arg("--list")
        .env("PATH", get_paths())
        .assert()
        .stdout(predicates::str::contains("hello"))
        .stdout(predicates::str::contains("bye"));
}

#[test]
#[cfg(not(unix))]
fn has_no_path() {
    // Call soroban with the PATH variable set to include just target/bin directory
    assert_cmd::Command::cargo_bin("stellar")
        .unwrap_or_else(|_| assert_cmd::Command::new("stellar"))
        .arg("hello")
        .env("PATH", target_bin())
        .assert()
        .stdout("Hello, world!\n");
}

#[test]
#[cfg(not(windows))]
fn has_no_path_failure() {
    // Call soroban with the PATH variable set to include just target/bin directory
    assert_cmd::Command::cargo_bin("stellar")
        .unwrap_or_else(|_| assert_cmd::Command::new("stellar"))
        .arg("hello")
        .assert()
        .stderr(predicates::str::contains("unrecognized subcommand 'hello'"));
}

fn target_bin() -> PathBuf {
    // Get the current working directory
    let current_dir = std::env::current_dir().unwrap();

    // Create a path to the target/bin directory
    current_dir
        .join("../../../target/bin")
        .canonicalize()
        .unwrap()
}

fn get_paths() -> OsString {
    let target_bin_path = target_bin();
    // Get the current PATH environment variable
    let path_key = std::env::var_os("PATH");
    if let Some(path_key) = path_key {
        // Create an iterator of paths from the PATH environment variable
        let current_paths = std::env::split_paths(&path_key);
        std::env::join_paths(current_paths.chain(vec![target_bin_path])).unwrap()
    } else {
        target_bin_path.into()
    }
}
