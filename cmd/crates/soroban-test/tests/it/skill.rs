use soroban_test::TestEnv;

#[test]
fn skill_prints_agent_guide() {
    let sandbox = TestEnv::default();
    sandbox
        .new_assert_cmd("skill")
        .assert()
        .success()
        .stdout(predicates::str::contains("network use"))
        .stdout(predicates::str::contains("--alias"))
        .stdout(predicates::str::contains("--id"));
}
