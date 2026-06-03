//! Auth mode for Soroban transaction simulation.
//!
//! Selects how the RPC handles authorization entries while simulating a
//! transaction. The variants map onto the RPC `simulateTransaction` `authMode`
//! parameter; leaving the argument unset omits the parameter and uses the RPC
//! default.

use clap::ValueEnum;

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum AuthMode {
    /// Validate the authorization entries already on the transaction.
    Enforce,
    /// Record authorization entries, requiring each to be rooted at the
    /// transaction's top-level operation.
    Root,
    /// Record all authorization entries, including non-root entries.
    #[value(name = "non-root")]
    NonRoot,
}

impl AuthMode {
    /// Map to the RPC `simulateTransaction` `authMode` parameter.
    pub fn to_rpc(self) -> soroban_rpc::AuthMode {
        match self {
            AuthMode::Enforce => soroban_rpc::AuthMode::Enforce,
            AuthMode::Root => soroban_rpc::AuthMode::Record,
            AuthMode::NonRoot => soroban_rpc::AuthMode::RecordAllowNonRoot,
        }
    }
}

/// Shared `--auth-mode` argument for commands that simulate Soroban
/// transactions.
///
/// The argument is optional: when unset, no `authMode` is sent and the RPC uses
/// its default (record with root mode if no authorization entries exist,
/// otherwise enforce the provided entries). This is also the only safe behavior
/// for envelopes whose operation is not `InvokeHostFunction`, since the RPC
/// rejects `authMode` for those.
#[derive(Debug, clap::Args, Clone, Default)]
#[group(skip)]
pub struct Args {
    /// Set the authorization mode for transaction simulation. When unset, the RPC
    /// default is used: record with the root mode if no authorization entries
    /// exist, otherwise enforce the provided entries. Should only be set for
    /// `InvokeHostFunction` transactions.
    #[arg(
        long,
        env = "STELLAR_AUTH_MODE",
        help_heading = crate::commands::HEADING_RPC,
    )]
    pub auth_mode: Option<AuthMode>,
}

impl Args {
    pub fn to_rpc(&self) -> Option<soroban_rpc::AuthMode> {
        self.auth_mode.map(AuthMode::to_rpc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unset_omits_rpc_auth_mode() {
        assert!(Args::default().to_rpc().is_none());
    }

    #[test]
    fn enforce_maps_to_enforce() {
        assert!(matches!(
            AuthMode::Enforce.to_rpc(),
            soroban_rpc::AuthMode::Enforce
        ));
    }

    #[test]
    fn root_maps_to_record() {
        assert!(matches!(
            AuthMode::Root.to_rpc(),
            soroban_rpc::AuthMode::Record
        ));
    }

    #[test]
    fn non_root_maps_to_record_allow_non_root() {
        assert!(matches!(
            AuthMode::NonRoot.to_rpc(),
            soroban_rpc::AuthMode::RecordAllowNonRoot
        ));
    }
}
