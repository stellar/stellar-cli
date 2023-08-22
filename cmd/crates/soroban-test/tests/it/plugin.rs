/*
This function calls the soroban executable via cargo and checks that the output
is correct. The PATH environment variable is set to include the target/bin
directory, so that the soroban executable can be found.
*/

use std::ffi::OsString;

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
fn has_no_path() {
    std::env::remove_var("PATH");
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

fn get_paths() -> OsString {
    // Get the current working directory
    let current_dir = std::env::current_dir().unwrap();

    // Create a path to the target/bin directory
    let target_bin_path = current_dir.join("../../../target/bin");
    // Get the current PATH environment variable
    let path_key = std::env::var_os("PATH").unwrap_or_default();
    // Create an iterator of paths from the PATH environment variable
    let current_paths = std::env::split_paths(&path_key);
    std::env::join_paths(current_paths.chain(vec![target_bin_path])).unwrap()
}
