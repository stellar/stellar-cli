use soroban_test::{AssertExt, TestEnv};

#[tokio::test]
async fn sep_53_sign_message_and_verify() {
    let sandbox = &TestEnv::new();

    let message = "Hello, World!";
    let expected_signature =
        "fO5dbYhXUhBMhe6kId/cuVq/AfEnHRHEvsP8vXh03M1uLpi5e46yO2Q8rEBzu3feXQewcQE5GArp88u6ePK6BA==";
    let wrong_signature =
        "CDU265Xs8y3OWbB/56H9jPgUss5G9A0qFuTqH2zs2YDgTm+++dIfmAEceFqB7bhfN3am59lCtDXrCtwH2k1GBA==";
    let secret_key = "SAKICEVQLYWGSOJS4WW7HZJWAHZVEEBS527LHK5V4MLJALYKICQCJXMW";
    let public_key = "GBXFXNDLV4LSWA4VB7YIL5GBD7BVNR22SGBTDKMO2SBZZHDXSKZYCP7L";
    let wrong_public_key = "GAREAZZQWHOCBJS236KIE3AWYBVFLSBK7E5UW3ICI3TCRWQKT5LNLCEZ";

    let output = sandbox
        .new_assert_cmd("message")
        .args(["sign", message, "--sign-with-key", secret_key])
        .assert()
        .success()
        .stdout_as_str();
    assert_eq!(output.trim(), expected_signature);

    sandbox
        .new_assert_cmd("message")
        .args([
            "verify",
            message,
            "--signature",
            expected_signature,
            "--public-key",
            public_key,
        ])
        .assert()
        .success();

    // wrong signature
    sandbox
        .new_assert_cmd("message")
        .args([
            "verify",
            message,
            "--signature",
            wrong_signature,
            "--public-key",
            public_key,
        ])
        .assert()
        .failure();

    // wrong public key
    sandbox
        .new_assert_cmd("message")
        .args([
            "verify",
            message,
            "--signature",
            expected_signature,
            "--public-key",
            wrong_public_key,
        ])
        .assert()
        .failure();
}

#[tokio::test]
async fn sep_53_sign_message_and_verify_stdin() {
    let sandbox = &TestEnv::new();

    let message = "Hello, World!";
    let expected_signature =
        "fO5dbYhXUhBMhe6kId/cuVq/AfEnHRHEvsP8vXh03M1uLpi5e46yO2Q8rEBzu3feXQewcQE5GArp88u6ePK6BA==";
    let secret_key = "SAKICEVQLYWGSOJS4WW7HZJWAHZVEEBS527LHK5V4MLJALYKICQCJXMW";
    let public_key = "GBXFXNDLV4LSWA4VB7YIL5GBD7BVNR22SGBTDKMO2SBZZHDXSKZYCP7L";

    // sandbox
    //     .new_assert_cmd("keys")
    //     .args(["add", alias_secret, "--secret-key", secret_key])
    //     .assert()
    //     .success();
    // sandbox
    //     .new_assert_cmd("keys")
    //     .args(["add", alias_public, "--public-key", public_key])
    //     .assert()
    //     .success();

    let output = sandbox
        .new_assert_cmd("message")
        .write_stdin(message)
        .args(["sign", "--sign-with-key", secret_key])
        .assert()
        .success()
        .stdout_as_str();
    assert_eq!(output.trim(), expected_signature);

    sandbox
        .new_assert_cmd("message")
        .write_stdin(message)
        .args([
            "verify",
            "--signature",
            expected_signature,
            "--public-key",
            public_key,
        ])
        .assert()
        .success();
}

#[tokio::test]
async fn sep_53_sign_message_and_verify_with_alias() {
    let sandbox = &TestEnv::new();

    let message = "Hello, World!";
    let expected_signature =
        "fO5dbYhXUhBMhe6kId/cuVq/AfEnHRHEvsP8vXh03M1uLpi5e46yO2Q8rEBzu3feXQewcQE5GArp88u6ePK6BA==";
    let public_key = "GBXFXNDLV4LSWA4VB7YIL5GBD7BVNR22SGBTDKMO2SBZZHDXSKZYCP7L";

    // generate a new secret "alice" and a public alias "bob" of the example pubkey
    sandbox
        .new_assert_cmd("keys")
        .args(["generate", "alice"])
        .assert()
        .success();
    sandbox
        .new_assert_cmd("keys")
        .args(["add", "bob", "--public-key", public_key])
        .assert()
        .success();

    // since this is randomly generated, just validate the output matches for alice
    let alice_signature = sandbox
        .new_assert_cmd("message")
        .write_stdin(message)
        .args(["sign", "--sign-with-key", "alice"])
        .assert()
        .success()
        .stdout_as_str();
    sandbox
        .new_assert_cmd("message")
        .write_stdin(message)
        .args([
            "verify",
            "--signature",
            &alice_signature,
            "--public-key",
            "alice",
        ])
        .assert()
        .success();

    // validate a public key alias works for validation
    sandbox
        .new_assert_cmd("message")
        .write_stdin(message)
        .args([
            "verify",
            "--signature",
            expected_signature,
            "--public-key",
            "bob",
        ])
        .assert()
        .success();
    sandbox
        .new_assert_cmd("message")
        .write_stdin(message)
        .args([
            "verify",
            "--signature",
            &alice_signature,
            "--public-key",
            "bob",
        ])
        .assert()
        .failure();
}
