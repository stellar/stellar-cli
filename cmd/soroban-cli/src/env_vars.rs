// List of environment variables used by the CLI.
// Most values come from `clap` env var aliases, but some are used directly.
// This list must include everything, even env vars that are secrets.
pub fn unprefixed() -> Vec<&'static str> {
    vec![
        "ACCOUNT",
        "ARCHIVE_URL",
        "CONFIG_HOME",
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

/// Returns true if the key is one of the supported env vars that should be shown in `stellar env`.
/// Uses an allow list approach to avoid showing any env vars that are not explicitly supported,
/// even if they start with the expected prefix.
pub fn is_visible(key: &str) -> bool {
    let visible = vec![
        "ACCOUNT",
        "ARCHIVE_URL",
        "CONFIG_HOME",
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
        "RPC_URL",
        "SEND",
        "SIGN_WITH_LAB",
        "SIGN_WITH_LEDGER",
    ];

    visible.iter().any(|suffix| key.ends_with(suffix))
}

pub fn prefixed(key: &str) -> Vec<String> {
    unprefixed()
        .iter()
        .map(|var| format!("{key}_{var}"))
        .collect::<Vec<String>>()
}
