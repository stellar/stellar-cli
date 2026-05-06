use soroban_test::TestEnv;

#[test]
fn decode() {
    let sandbox = TestEnv::default();
    sandbox
        .new_assert_cmd("strkey")
        .args([
            "decode",
            "GAZTAML6YJA5PGKDXONKPSHKL6FYT6OG5I2R7YB7R3B5CETRG7KIJONK",
        ])
        .assert()
        .success()
        .stdout(
            r#"{
  "public_key_ed25519": "3330317ec241d79943bb9aa7c8ea5f8b89f9c6ea351fe03f8ec3d1127137d484"
}
"#,
        );
}

#[test]
fn encode() {
    let sandbox = TestEnv::default();
    sandbox
        .new_assert_cmd("strkey")
        .args(["encode", r#"{"public_key_ed25519":"3330317ec241d79943bb9aa7c8ea5f8b89f9c6ea351fe03f8ec3d1127137d484"}"#])
        .assert()
        .success()
        .stdout("GAZTAML6YJA5PGKDXONKPSHKL6FYT6OG5I2R7YB7R3B5CETRG7KIJONK\n");
}
