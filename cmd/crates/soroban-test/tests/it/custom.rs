#[test]
fn call_custom() {
    let current_dir = std::env::current_dir().unwrap();
    let install_path = current_dir.join("tests/fixtures/hello");

    let root = current_dir
        .join("../../../target/.bin")
        .canonicalize()
        .unwrap();
    assert_cmd::Command::new("cargo")
        .args([
            "install",
            "--path",
            install_path.to_str().unwrap(),
            "--root",
            &format!("{root:?}"),
        ])
        .assert()
        .success();
    let bin_path = root.join("bin");
    let path_key = std::env::var_os("PATH").unwrap();
    let current_paths = std::env::split_paths(&path_key);
    let paths = std::env::join_paths(current_paths.chain(vec![bin_path])).unwrap();

    assert_cmd::Command::cargo_bin("soroban")
        .unwrap()
        .arg("hello")
        .env("PATH", paths)
        .assert()
        .stdout("Hello, world!\n");
}
