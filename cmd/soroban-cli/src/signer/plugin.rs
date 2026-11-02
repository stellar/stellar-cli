//! Signs auth entries for smart accounts (contract addresses) by running an
//! external `stellar-signer-<name>` binary, since only the account's own
//! `__check_auth` knows what its signature looks like.
//!
//! The plugin reads one JSON [`SignAuthRequest`] on stdin and writes the
//! credential's `signature` as a base64 XDR `ScVal` on stdout. Its stderr is
//! the user's terminal, so it can prompt. `STELLAR_BIN` names the running
//! `stellar` binary, so the plugin can call e.g. `$STELLAR_BIN keys secret`.

use std::{
    collections::BTreeMap,
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
};

use serde::Serialize;

use crate::{
    utils::XDR_DEPTH_LIMIT,
    xdr::{self, Limits, ReadXdr, ScVal, SorobanAuthorizedInvocation, WriteXdr},
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("signer plugin `{bin}` not found on PATH")]
    NotFound { bin: String },
    #[error("failed to run signer plugin `{bin}`: {source}")]
    Spawn { bin: String, source: std::io::Error },
    #[error("signer plugin `{bin}` failed ({status})")]
    Failed {
        bin: String,
        status: std::process::ExitStatus,
    },
    #[error("signer plugin `{bin}` returned an invalid signature, expected a base64 XDR ScVal: {source}")]
    InvalidOutput { bin: String, source: xdr::Error },
    #[error(transparent)]
    Xdr(#[from] xdr::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub struct PluginSigner {
    pub address: stellar_strkey::Contract,
    pub plugin: String,
    pub args: BTreeMap<String, String>,
}

/// What a plugin is asked to sign. Mirrors what `__check_auth` receives:
/// `payload` is its `signature_payload`, and `root_invocation` its auth
/// context.
#[derive(Serialize, Debug)]
pub struct SignAuthRequest<'a> {
    pub mode: &'static str,
    pub network_passphrase: &'a str,
    pub address: String,
    pub nonce: i64,
    pub signature_expiration_ledger: u32,
    /// `address` for an `Address` credential, `address_v2` for `AddressV2`
    pub credential_kind: &'static str,
    /// Hex sha256 of the `HashIdPreimage` for the credential kind
    pub payload: String,
    /// Base64 XDR `SorobanAuthorizedInvocation`
    pub root_invocation: String,
    pub args: &'a BTreeMap<String, String>,
}

impl PluginSigner {
    pub fn bin_name(&self) -> String {
        format!("stellar-signer-{}", self.plugin)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn sign_auth(
        &self,
        network_passphrase: &str,
        nonce: i64,
        signature_expiration_ledger: u32,
        is_v2: bool,
        payload: [u8; 32],
        invocation: &SorobanAuthorizedInvocation,
    ) -> Result<ScVal, Error> {
        let request = SignAuthRequest {
            mode: "sign_auth",
            network_passphrase,
            address: format!("{}", self.address),
            nonce,
            signature_expiration_ledger,
            credential_kind: if is_v2 { "address_v2" } else { "address" },
            payload: hex::encode(payload),
            root_invocation: invocation.to_xdr_base64(Limits::depth(XDR_DEPTH_LIMIT))?,
            args: &self.args,
        };
        let stdout = self.run(&serde_json::to_vec(&request)?)?;
        ScVal::from_xdr_base64(stdout.trim(), Limits::depth(XDR_DEPTH_LIMIT)).map_err(|source| {
            Error::InvalidOutput {
                bin: self.bin_name(),
                source,
            }
        })
    }

    fn run(&self, input: &[u8]) -> Result<String, Error> {
        let bin = self.bin_name();
        let path: PathBuf = which::which(&bin).map_err(|_| Error::NotFound { bin: bin.clone() })?;
        let spawn_err = |source| Error::Spawn {
            bin: bin.clone(),
            source,
        };
        let mut cmd = Command::new(path);
        if let Ok(exe) = std::env::current_exe() {
            cmd.env("STELLAR_BIN", exe);
        }
        let mut child = cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(spawn_err)?;
        child
            .stdin
            .take()
            .expect("stdin is piped")
            .write_all(input)
            .map_err(spawn_err)?;
        let output = child.wait_with_output().map_err(spawn_err)?;
        if !output.status.success() {
            return Err(Error::Failed {
                bin,
                status: output.status,
            });
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }
}
