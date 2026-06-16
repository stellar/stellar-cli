/// Asserts that `output` contains no terminal-escape / control characters
/// other than `\n` and `\t`.
///
/// Used by regression tests that verify `sanitize(...)` is correctly applied
/// to attacker-controlled spec / event strings before they reach Display.
pub fn assert_no_control_chars(output: &str) {
    let bad: Vec<char> = output
        .chars()
        .filter(|c| c.is_control() && *c != '\n' && *c != '\t')
        .collect();
    assert!(
        bad.is_empty(),
        "control chars survived: {bad:?} in output {output:?}"
    );
}
