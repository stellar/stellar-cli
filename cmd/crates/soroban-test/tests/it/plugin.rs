/*
This function calls the soroban executable via cargo and checks that the output
is correct. The PATH environment variable is set to include the target/bin
directory, so that the soroban executable can be found.
*/

use std::{ffi::OsString, path::PathBuf};

#[test]
fn soroban_hello() {
    // Add the target/bin directory to the iterator of paths
    let paths = get_paths();
    // Call soroban with the PATH variable set to include the target/bin directory
    assert_cmd::Command::cargo_bin("soroban")
        .unwrap_or_else(|_| assert_cmd::Command::new("soroban"))
        .arg("hello")
        .env("PATH", &paths)
        .assert()
        .stdout("Hello, world!\n");
}

#[test]
fn list() {
    // Call `soroban --list` with the PATH variable set to include the target/bin directory
    assert_cmd::Command::cargo_bin("soroban")
        .unwrap_or_else(|_| assert_cmd::Command::new("soroban"))
        .arg("--list")
        .env("PATH", get_paths())
        .assert()
        .stdout(predicates::str::contains("hello"));
}

#[test]
#[cfg(not(unix))]
fn has_no_path() {
    // Call soroban with the PATH variable set to include just target/bin directory
    assert_cmd::Command::cargo_bin("soroban")
        .unwrap_or_else(|_| assert_cmd::Command::new("soroban"))
        .arg("hello")
        .env("PATH", target_bin())
        .assert()
        .stdout("Hello, world!\n");
}

#[test]
fn has_no_path_failure() {
    // Call soroban with the PATH variable set to include just target/bin directory
    assert_cmd::Command::cargo_bin("soroban")
        .unwrap_or_else(|_| assert_cmd::Command::new("soroban"))
        .arg("hello")
        .assert()
        .stderr(predicates::str::contains("error: no such command: `hello`"));
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
