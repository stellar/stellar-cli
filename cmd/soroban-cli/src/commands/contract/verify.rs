use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;

use clap::Parser;
use sha2::{Digest, Sha256};
use soroban_spec_tools::contract::{self, Spec};
use stellar_xdr::curr::{ScMetaEntry, ScMetaV0};

use super::build;
use super::info::shared::{self, fetch, Contract, Fetched};
use crate::commands::container::shared::Args as ContainerArgs;
use crate::commands::{global, version};
use crate::print::Print;

/// Verify a wasm by rebuilding it inside the Docker image recorded in its metadata.
///
/// Succeeds only if the rebuilt artifact is byte-identical to the input.
/// User is responsible for checking out the matching commit before running.
#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub common: shared::Args,

    /// Source tree (Cargo.toml directory) to rebuild from. Defaults to cwd.
    #[arg(long, default_value = ".")]
    pub source: PathBuf,

    /// Override the docker image read from the contract metadata. For debugging only.
    #[arg(long, value_name = "IMAGE", help_heading = "Advanced")]
    pub docker: Option<String>,

    #[command(flatten)]
    pub container_args: ContainerArgs,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Shared(#[from] shared::Error),
    #[error(transparent)]
    Spec(#[from] contract::Error),
    #[error(transparent)]
    Build(#[from] build::Error),
    #[error("stellar asset contract has no source to verify")]
    StellarAssetContract,
    #[error("required '{0}' meta entry not found in contract; rebuild the wasm with `stellar contract build --docker` to make it verifiable")]
    MissingMeta(&'static str),
    #[error("CLI version mismatch: contract says '{expected}', running CLI is '{actual}'. Install the matching CLI version and re-run.")]
    CliVersionMismatch { expected: String, actual: String },
    #[error("{}", format_mismatch(expected, produced))]
    Mismatch {
        expected: String,
        produced: Vec<(String, String, PathBuf)>,
    },
    #[error(
        "no Cargo.toml found at {0}; pass --source <path> to point at the contract's source tree"
    )]
    SourceNotFound(PathBuf),
    #[error("reading rebuilt wasm: {0}")]
    ReadingRebuilt(std::io::Error),
}

fn format_mismatch(expected: &str, produced: &[(String, String, PathBuf)]) -> String {
    let mut s = format!(
        "verification failed: rebuilt wasm does not match (expected sha256 {expected}).\nproduced:"
    );
    for (name, hash, path) in produced {
        let _ = write!(s, "\n  {name}  sha256:{hash}  {}", path.display());
    }
    s
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let print = Print::new(global_args.quiet);

        let Fetched { contract, .. } = fetch(&self.common, &print).await?;
        let wasm_bytes = match contract {
            Contract::Wasm { wasm_bytes } => wasm_bytes,
            Contract::StellarAssetContract => return Err(Error::StellarAssetContract),
        };
        let original_hash = hex::encode(Sha256::digest(&wasm_bytes));
        print.infoln(format!("Original wasm sha256: {original_hash}"));

        let spec = Spec::new(&wasm_bytes)?;
        let cliver = find_meta(&spec.meta, "cliver").ok_or(Error::MissingMeta("cliver"))?;
        let running = version::one_line();
        if cliver != running {
            return Err(Error::CliVersionMismatch {
                expected: cliver,
                actual: running,
            });
        }
        let bldimg = match &self.docker {
            Some(image) => image.clone(),
            None => find_meta(&spec.meta, "bldimg").ok_or(Error::MissingMeta("bldimg"))?,
        };
        let rsver = find_meta(&spec.meta, "rsver").ok_or(Error::MissingMeta("rsver"))?;

        let manifest_path = self.source.join("Cargo.toml");
        if !manifest_path.exists() {
            return Err(Error::SourceNotFound(self.source.clone()));
        }

        let build_cmd = build::Cmd {
            manifest_path: Some(manifest_path),
            docker: Some(bldimg),
            container_args: self.container_args.clone(),
            rustup_toolchain: Some(rsver),
            ..build::Cmd::default()
        };
        let built = build_cmd.run(global_args).await?;

        let mut produced = Vec::with_capacity(built.len());
        let mut matched = None;
        for c in &built {
            let bytes = fs::read(&c.path).map_err(Error::ReadingRebuilt)?;
            let hash = hex::encode(Sha256::digest(&bytes));
            if hash == original_hash {
                matched = Some(c.name.clone());
            }
            produced.push((c.name.clone(), hash, c.path.clone()));
        }

        // Verdict bypasses --quiet because pass/fail is this command's primary output.
        if let Some(name) = matched {
            eprintln!("✅ Verified: rebuilt wasm matches (sha256 {original_hash}) — {name}");
            Ok(())
        } else {
            eprintln!("⚠ Verification failed: rebuilt wasm does not match original.");
            Err(Error::Mismatch {
                expected: original_hash,
                produced,
            })
        }
    }
}

fn find_meta(meta: &[ScMetaEntry], key: &str) -> Option<String> {
    meta.iter().find_map(|entry| {
        let ScMetaEntry::ScMetaV0(ScMetaV0 { key: k, val }) = entry;
        (k.to_string() == key).then(|| val.to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(key: &str, val: &str) -> ScMetaEntry {
        ScMetaEntry::ScMetaV0(ScMetaV0 {
            key: key.to_string().try_into().unwrap(),
            val: val.to_string().try_into().unwrap(),
        })
    }

    #[test]
    fn find_meta_returns_value_for_exact_key() {
        let meta = vec![
            entry("bldimg2", "wrong"),
            entry("cliver", "v1"),
            entry("bldimg", "img@sha256:abc"),
        ];
        assert_eq!(find_meta(&meta, "cliver"), Some("v1".to_string()));
        assert_eq!(
            find_meta(&meta, "bldimg"),
            Some("img@sha256:abc".to_string())
        );
        assert_eq!(find_meta(&meta, "missing"), None);
    }
}
