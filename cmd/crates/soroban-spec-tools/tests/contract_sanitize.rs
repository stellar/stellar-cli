use soroban_spec_tools::contract::Spec;
use std::fs;

/// Mirrors `stellar contract inspect`: fs::read -> Spec::new -> spec.to_string().
#[test]
fn inspect_strips_control_characters() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/control_characters.wasm"
    );
    let bytes = fs::read(path).expect("fixture wasm should be readable");
    let spec = Spec::new(&bytes).expect("wasm should parse without error");
    let output = spec.to_string();

    let bad_chars: Vec<char> = output
        .chars()
        .filter(|c| c.is_control() && *c != '\n' && *c != '\t')
        .collect();
    assert!(
        bad_chars.is_empty(),
        "inspect output contains unexpected control characters {bad_chars:?}:\n{output:?}"
    );
}
