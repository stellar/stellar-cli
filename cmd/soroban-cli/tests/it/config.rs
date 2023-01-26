use crate::util::{temp_dir, Sandbox, SorobanCommand};
use std::{fs, path::Path};

#[test]
fn set_and_remove_network() {
    let dir = temp_dir();

    let _ = Sandbox::new_cmd("config")
        .current_dir(&dir)
        .arg("network")
        .arg("add")
        .arg("--rpc-url")
        .arg("https://127.0.0.1")
        .arg("--network-passphrase")
        .arg("Local Sandbox Stellar Network ; September 2022")
        .arg("local")
        .assert()
        .success();
    let file = std::fs::read_dir(dir.join(".soroban/networks"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(file.file_name().to_str().unwrap(), "local.toml");

    Sandbox::new_cmd("config")
        .current_dir(&dir)
        .arg("network")
        .arg("ls")
        .assert()
        .stdout("local");

    Sandbox::new_cmd("config")
        .current_dir(&dir)
        .arg("network")
        .arg("rm")
        .arg("local")
        .assert()
        .stdout("");
    Sandbox::new_cmd("config")
        .current_dir(&dir)
        .arg("network")
        .arg("ls")
        .assert()
        .stdout("");
}

fn add_network(dir: &Path, name: &str) {
    Sandbox::new_cmd("config")
        .current_dir(dir)
        .arg("network")
        .arg("add")
        .arg("--rpc-url")
        .arg("https://127.0.0.1")
        .arg("--network-passphrase")
        .arg("Local Sandbox Stellar Network ; September 2022")
        .arg(name)
        .assert()
        .success();
}

fn add_network_global(dir: &Path, name: &str) {
    Sandbox::new_cmd("config")
        .env("HOME", dir.to_str().unwrap())
        .arg("network")
        .arg("add")
        .arg("--global")
        .arg("--rpc-url")
        .arg("https://127.0.0.1")
        .arg("--network-passphrase")
        .arg("Local Sandbox Stellar Network ; September 2022")
        .arg(name)
        .assert()
        .success();
}

#[test]
fn set_and_remove_global_network() {
    let dir = temp_dir();

    // add_network_global(&dir, "local");

    // Sandbox::new_cmd("config")
    //     .env("HOME", dir.to_str().unwrap())
    //     .arg("network")
    //     .arg("ls")
    //     .assert()
    //     .stdout("local");

    // Sandbox::new_cmd("config")
    //     .env("HOME", dir.to_str().unwrap())
    //     .arg("network")
    //     .arg("rm")
    //     .arg("--global")
    //     .arg("local")
    //     .assert()
    //     .stdout("");

    // Sandbox::new_cmd("config")
    //     .env("HOME", dir.to_str().unwrap())
    //     .arg("network")
    //     .arg("ls")
    //     .assert()
    //     .stdout("");
}

#[test]
fn mulitple_networks() {
    let dir = temp_dir();

    add_network(&dir, "local");
    add_network(&dir, "local2");

    Sandbox::new_cmd("config")
        .current_dir(&dir)
        .arg("network")
        .arg("ls")
        .assert()
        .stdout("local\nlocal2");

    Sandbox::new_cmd("config")
        .current_dir(&dir)
        .arg("network")
        .arg("rm")
        .arg("local")
        .assert();
    Sandbox::new_cmd("config")
        .current_dir(&dir)
        .arg("network")
        .arg("ls")
        .assert()
        .stdout("local2");

    let sub_dir = dir.join("sub_directory");
    fs::create_dir(&sub_dir).unwrap();
    add_network(&sub_dir, "local3");

    Sandbox::new_cmd("config")
        .current_dir(&dir)
        .arg("network")
        .arg("ls")
        .assert()
        .stdout("local2\nlocal3");
}

#[test]
fn read_identity() {
    Sandbox::new_cmd("config")
        .arg("identity")
        .arg("ls")
        .assert()
        .stdout("test_id");
}
