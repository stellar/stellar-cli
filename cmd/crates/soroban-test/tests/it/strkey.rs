use soroban_test::TestEnv;

#[test]
fn decode() {
    let sandbox = TestEnv::default();
    sandbox
        .new_assert_cmd("strkey")
        .args(["decode"])
        .write_stdin("GAZTAML6YJA5PGKDXONKPSHKL6FYT6OG5I2R7YB7R3B5CETRG7KIJONK")
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
fn decode_arg() {
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
        .args(["encode"])
        .write_stdin(r#"{"public_key_ed25519":"3330317ec241d79943bb9aa7c8ea5f8b89f9c6ea351fe03f8ec3d1127137d484"}"#)
        .assert()
        .success()
        .stdout("GAZTAML6YJA5PGKDXONKPSHKL6FYT6OG5I2R7YB7R3B5CETRG7KIJONK\n");
}

#[test]
fn encode_arg() {
    let sandbox = TestEnv::default();
    sandbox
        .new_assert_cmd("strkey")
        .args(["encode", r#"{"public_key_ed25519":"3330317ec241d79943bb9aa7c8ea5f8b89f9c6ea351fe03f8ec3d1127137d484"}"#])
        .assert()
        .success()
        .stdout("GAZTAML6YJA5PGKDXONKPSHKL6FYT6OG5I2R7YB7R3B5CETRG7KIJONK\n");
}

// Input passed as an argument is handled the same as input from stdin,
// including for private keys and invalid input.
#[test]
fn arg_same_as_stdin() {
    let sandbox = TestEnv::default();
    for args in [
        [
            "decode",
            "SBF5HLRREHMS36XZNTUSKZ6FTXDZGNXOHF4EXKUL5UCWZLPBX3NGJ4BH",
        ],
        [
            "encode",
            r#"{"private_key_ed25519":"0000000000000000000000000000000000000000000000000000000000000000"}"#,
        ],
        ["decode", "invalid"],
        ["encode", "invalid"],
    ] {
        let [cmd, input] = args;
        let from_stdin = sandbox
            .new_assert_cmd("strkey")
            .arg(cmd)
            .write_stdin(input)
            .output()
            .unwrap();
        let from_arg = sandbox
            .new_assert_cmd("strkey")
            .args(args)
            .output()
            .unwrap();
        assert_eq!(from_arg, from_stdin, "{args:?}");
    }
}
