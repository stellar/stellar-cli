// List of environment variables used by the CLI.
// Most values come from `clap` env var aliases, but some are used directly.
// This list must include everything, even env vars that are secrets.
pub fn unprefixed() -> Vec<&'static str> {
    vec![
        "ACCOUNT",
        "ARCHIVE_URL",
        "CONFIG_HOME",
        "CONTAINER_ENGINE",
        "CONTRACT_ID",
        "DATA_HOME",
        "FEE",
        "INCLUSION_FEE",
        "INVOKE_VIEW",
        "NETWORK",
        "NETWORK_PASSPHRASE",
        "NO_CACHE",
        "NO_UPDATE_CHECK",
        "OPERATION_SOURCE_ACCOUNT",
        "RPC_HEADERS",
        "RPC_URL",
        "SECRET_KEY",
        "SEND",
        "SIGN_WITH_KEY",
        "SIGN_WITH_LAB",
        "SIGN_WITH_LEDGER",
    ]
}

/// Unprefixed names of env vars that are safe to display in plain text.
const VISIBLE: &[&str] = &[
    "ACCOUNT",
    "CONFIG_HOME",
    "CONTAINER_ENGINE",
    "CONTRACT_ID",
    "DATA_HOME",
    "FEE",
    "INCLUSION_FEE",
    "INVOKE_VIEW",
    "NETWORK",
    "NETWORK_PASSPHRASE",
    "NO_CACHE",
    "NO_UPDATE_CHECK",
    "OPERATION_SOURCE_ACCOUNT",
    "SEND",
    "SIGN_WITH_LAB",
    "SIGN_WITH_LEDGER",
];

/// Returns true if the key should be concealed in `stellar env` output, i.e. it is not in the
/// allow list of vars that are safe to display. Using an allow list ensures unknown vars are
/// concealed by default, even if they start with the expected prefix.
pub fn is_concealed(key: &str) -> bool {
    let name = key
        .strip_prefix("STELLAR_")
        .or_else(|| key.strip_prefix("SOROBAN_"))
        .unwrap_or(key);
    !VISIBLE.contains(&name)
}

pub fn prefixed(key: &str) -> Vec<String> {
    unprefixed()
        .iter()
        .map(|var| format!("{key}_{var}"))
        .collect::<Vec<String>>()
}
