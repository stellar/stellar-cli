// List of environment variables used by the CLI.
// Most values come from `clap` env var aliases, but some are used directly.
// This list must include everything, even env vars that are secrets.
pub fn unprefixed() -> Vec<&'static str> {
    vec![
        "ACCOUNT",
        "ARCHIVE_URL",
        "AUTH_MODE",
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
        "RESOURCE_FEE",
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
    "AUTH_MODE",
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
    "RESOURCE_FEE",
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

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    fn collect_env_aliases(cmd: &clap::Command, out: &mut Vec<String>) {
        for arg in cmd.get_arguments() {
            if let Some(env) = arg.get_env() {
                out.push(env.to_string_lossy().into_owned());
            }
        }
        for sub in cmd.get_subcommands() {
            collect_env_aliases(sub, out);
        }
    }

    #[test]
    fn every_declared_env_var_is_registered() {
        let mut aliases = Vec::new();
        collect_env_aliases(&crate::commands::Root::command(), &mut aliases);

        let registered = unprefixed();
        for alias in aliases {
            // Only STELLAR_/SOROBAN_ vars are governed here; third-party vars like
            // DOCKER_HOST are out of scope.
            let Some(name) = alias
                .strip_prefix("STELLAR_")
                .or_else(|| alias.strip_prefix("SOROBAN_"))
            else {
                continue;
            };
            assert!(
                registered.contains(&name),
                "{alias} is wired to a CLI argument but missing from env_vars::unprefixed(); \
                 add it so the display allow list keeps it concealed by default"
            );
        }
    }
}
