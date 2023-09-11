use soroban_test::TestEnv;
use std::path::PathBuf;

#[test]
#[cfg_attr(target_os = "windows", ignore)]
fn lab_xdr_decode() {
    let sandbox = TestEnv::default();
    let cargo_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let ref_xdr_filename = cargo_dir.join("tests/it/lab_test_transaction_envelope.txt");
    let ref_xdr = std::fs::read_to_string(ref_xdr_filename.clone()).unwrap();

    let cmd = sandbox
        .new_assert_cmd("lab")
        .arg("xdr")
        .arg("dec")
        .arg("--type")
        .arg("TransactionEnvelope")
        .arg("--xdr")
        .arg("AAAAAgAAAABzdv3ojkzWHMD7KUoXhrPx0GH18vHKV0ZfqpMiEblG1gAAAGQAAAAAAAAAAQAAAAEAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAEAAAAAAAAAAAAAAADRjwIQ/2zB8tzxMB+71MMO4RoHWCBoTUcd+J0PEBHqKAAAAOjUpRAAAAAAAAAAAAERuUbWAAAAQKAEpum2TGh/P2K0/eOxeXw1eGEG5fl/Ft2a/j7YUN+H3XNjkFAfYnJvfpmvTsNYqPsoHKufgRpDmJuAhd0xJgk=")
        .assert()
        .success();
    let stdout = String::from_utf8(cmd.get_output().clone().stdout).unwrap();
    if ref_xdr.is_empty() {
        std::fs::write(ref_xdr_filename, stdout.clone()).unwrap();
    }
    assert_eq!(stdout, ref_xdr);
}
