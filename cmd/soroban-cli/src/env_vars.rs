// List of environment variables used by the CLI.
// Most values come from `clap` env var aliases, but some are used directly.
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
        "SEND",
        "SIGN_WITH_LAB",
        "SIGN_WITH_LEDGER",
    ]
}

pub fn prefixed(key: &str) -> Vec<String> {
    unprefixed()
        .iter()
        .map(|var| format!("{key}_{var}"))
        .collect::<Vec<String>>()
}
