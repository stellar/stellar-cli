use soroban_test::TestEnv;

// Every contract subcommand that references an existing contract should expose
// `--contract-id` as the canonical flag with `--id` kept as an alias, matching
// `stellar contract info`.
fn assert_contract_id_flag(subcommand: &[&str]) {
    let sandbox = TestEnv::default();
    let help = sandbox
        .new_assert_cmd("contract")
        .args(subcommand)
        .arg("--help")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let help = String::from_utf8(help).unwrap();

    assert!(
        help.contains("--contract-id"),
        "`contract {}` help is missing --contract-id:\n{help}",
        subcommand.join(" ")
    );
    assert!(
        help.contains("--id"),
        "`contract {}` help is missing the --id alias:\n{help}",
        subcommand.join(" ")
    );
}

#[test]
fn contract_id_flag_is_consistent_across_commands() {
    for subcommand in [
        &["invoke"][..],
        &["fetch"][..],
        &["read"][..],
        &["extend"][..],
        &["restore"][..],
        &["info", "interface"][..],
        &["alias", "add"][..],
    ] {
        assert_contract_id_flag(subcommand);
    }
}
