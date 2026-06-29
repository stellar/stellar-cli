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

#[tokio::test]
async fn raw_sign_message_and_verify() {
    let sandbox = &TestEnv::new();

    let message = "challenge-1:abc123";
    let secret_key = "SAKICEVQLYWGSOJS4WW7HZJWAHZVEEBS527LHK5V4MLJALYKICQCJXMW";
    let public_key = "GBXFXNDLV4LSWA4VB7YIL5GBD7BVNR22SGBTDKMO2SBZZHDXSKZYCP7L";

    // --raw signs the exact bytes (no SEP-53 prefix/hash) and outputs hex.
    let signature = sandbox
        .new_assert_cmd("message")
        .args(["sign", message, "--sign-with-key", secret_key, "--raw"])
        .assert()
        .success()
        .stdout_as_str();
    let signature = signature.trim();
    assert_eq!(signature.len(), 128, "raw signature is 128 hex chars");
    assert!(signature.chars().all(|c| c.is_ascii_hexdigit()));

    sandbox
        .new_assert_cmd("message")
        .args([
            "verify",
            message,
            "--raw",
            "--signature",
            signature,
            "--public-key",
            public_key,
        ])
        .assert()
        .success();

    // The raw signature must not validate on the SEP-53 path (it has no prefix
    // or hashing), so verifying without --raw fails.
    sandbox
        .new_assert_cmd("message")
        .args([
            "verify",
            message,
            "--signature",
            signature,
            "--public-key",
            public_key,
        ])
        .assert()
        .failure();
}

#[tokio::test]
async fn raw_sign_preserves_stdin_trailing_newline() {
    let sandbox = &TestEnv::new();

    let secret_key = "SAKICEVQLYWGSOJS4WW7HZJWAHZVEEBS527LHK5V4MLJALYKICQCJXMW";

    // Signing exact bytes with a trailing newline (positional arg, no trimming).
    let arg_sig = sandbox
        .new_assert_cmd("message")
        .args(["sign", "payload\n", "--sign-with-key", secret_key, "--raw"])
        .assert()
        .success()
        .stdout_as_str();

    // The same bytes over stdin must produce the same signature: the raw path
    // must not strip the trailing newline.
    let stdin_sig = sandbox
        .new_assert_cmd("message")
        .write_stdin("payload\n")
        .args(["sign", "--sign-with-key", secret_key, "--raw"])
        .assert()
        .success()
        .stdout_as_str();

    assert_eq!(arg_sig.trim(), stdin_sig.trim());

    // And it must differ from signing the trimmed "payload" bytes.
    let trimmed_sig = sandbox
        .new_assert_cmd("message")
        .args(["sign", "payload", "--sign-with-key", secret_key, "--raw"])
        .assert()
        .success()
        .stdout_as_str();

    assert_ne!(stdin_sig.trim(), trimmed_sig.trim());
}

#[test]
fn message_sign_does_not_leak_secret_in_error_output() {
    let sandbox = TestEnv::default();
    let malformed =
        "kite urban olympic result lunch box duck abandon abandon abandon abandon about";

    let output = sandbox
        .new_assert_cmd("message")
        .args(["sign", "hello", "--sign-with-key", malformed])
        .assert()
        .failure();

    let stderr = String::from_utf8_lossy(&output.get_output().stderr).into_owned();
    assert!(
        !stderr.contains(malformed),
        "stderr must not contain the raw signing key, got: {stderr:?}"
    );
}

#[test]
fn message_sign_does_not_echo_message_to_stderr() {
    let sandbox = TestEnv::default();
    let secret_key = "SAKICEVQLYWGSOJS4WW7HZJWAHZVEEBS527LHK5V4MLJALYKICQCJXMW";
    let secret_message = "TOP_SECRET_TOKEN_abc123_DO_NOT_LEAK";

    let output = sandbox
        .new_assert_cmd("message")
        .args(["sign", secret_message, "--sign-with-key", secret_key])
        .assert()
        .success();
    let stderr = String::from_utf8_lossy(&output.get_output().stderr).into_owned();
    assert!(
        !stderr.contains(secret_message),
        "stderr must not echo the message (arg input), got: {stderr:?}"
    );

    let output = sandbox
        .new_assert_cmd("message")
        .write_stdin(secret_message)
        .args(["sign", "--sign-with-key", secret_key])
        .assert()
        .success();
    let stderr = String::from_utf8_lossy(&output.get_output().stderr).into_owned();
    assert!(
        !stderr.contains(secret_message),
        "stderr must not echo the message (stdin input), got: {stderr:?}"
    );
}
