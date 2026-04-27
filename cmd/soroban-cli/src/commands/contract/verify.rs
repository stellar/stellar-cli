use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;

use clap::Parser;
use sha2::{Digest, Sha256};
use soroban_spec_tools::contract::{self, Spec};
use stellar_xdr::curr::{ScMetaEntry, ScMetaV0};

use crate::commands::container::shared::Args as ContainerArgs;
use crate::commands::global;
use crate::commands::version;
use crate::print::Print;

use super::build;
use super::info::shared::{self, fetch, Contract, Fetched};

/// Verify that a wasm matches what would be produced by building its source.
///
/// Re-runs the build inside the same Docker image (digest) recorded in the
/// wasm's contract metadata and compares the resulting wasm hash. Succeeds
/// only if the rebuilt artifact is byte-identical.
///
/// Verify rebuilds from --source (default: current directory). The user is
/// responsible for checking out the right commit before running verify.
#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    /// Source of the wasm to verify. Provide one of --wasm, --wasm-hash, --contract-id.
    #[command(flatten)]
    pub common: shared::Args,

    /// Path to the source tree (Cargo.toml directory) used to rebuild.
    /// Defaults to current working directory.
    #[arg(long, default_value = ".")]
    pub source: PathBuf,

    /// Override the docker image read from the contract metadata.
    /// Use only for debugging — overriding will normally cause a hash mismatch.
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
    #[error("CLI version mismatch: contract metadata says '{expected}', running CLI is '{actual}'.\nInstall the matching CLI version and re-run `stellar contract verify`.")]
    CliVersionMismatch { expected: String, actual: String },
    #[error("{}", format_mismatch(expected, produced))]
    Mismatch {
        expected: String,
        produced: Vec<(String, String, PathBuf)>,
    },
    #[error("no Cargo.toml found at {0}; pass --source <path> to point at the contract's source tree")]
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
        let bldimg = match &self.docker {
            Some(image) => image.clone(),
            None => find_meta(&spec.meta, "bldimg").ok_or(Error::MissingMeta("bldimg"))?,
        };

        let running_cliver = version::one_line();
        if cliver != running_cliver {
            return Err(Error::CliVersionMismatch {
                expected: cliver,
                actual: running_cliver,
            });
        }
        print.infoln(format!("CLI version matches: {running_cliver}"));

        let manifest_path = self.source.join("Cargo.toml");
        if !manifest_path.exists() {
            return Err(Error::SourceNotFound(self.source.clone()));
        }

        print.infoln(format!("Rebuilding with docker image {bldimg}..."));
        let build_cmd = build::Cmd {
            manifest_path: Some(manifest_path),
            docker: Some(bldimg),
            container_args: self.container_args.clone(),
            ..build::Cmd::default()
        };
        let built = build_cmd.run(global_args).await?;

        let mut produced: Vec<(String, String, PathBuf)> = Vec::with_capacity(built.len());
        let mut matched: Option<String> = None;
        for c in &built {
            let bytes = fs::read(&c.path).map_err(Error::ReadingRebuilt)?;
            let hash = hex::encode(Sha256::digest(&bytes));
            if hash == original_hash {
                matched = Some(c.name.clone());
            }
            produced.push((c.name.clone(), hash, c.path.clone()));
        }

        if let Some(name) = matched {
            // Intentional: bypasses --quiet because the pass/fail verdict is the primary output of this command.
            eprintln!(
                "✅ Verified: rebuilt wasm matches the original (sha256 {original_hash}) — {name}"
            );
            Ok(())
        } else {
            // Intentional: bypasses --quiet because the pass/fail verdict is the primary output of this command.
            eprintln!("⚠ Verification failed: rebuilt wasm does not match original.");
            eprintln!("   Built artifacts:");
            for (name, hash, path) in &produced {
                eprintln!("     {name}  sha256:{hash}  {}", path.display());
            }
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
        if k.to_string() == key {
            Some(val.to_string())
        } else {
            None
        }
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
    fn find_meta_first_index() {
        let meta = vec![entry("cliver", "v1"), entry("bldimg", "img@sha256:abc")];
        assert_eq!(find_meta(&meta, "cliver"), Some("v1".to_string()));
    }

    #[test]
    fn find_meta_later_index() {
        let meta = vec![
            entry("cliver", "v1"),
            entry("other", "x"),
            entry("bldimg", "img@sha256:abc"),
        ];
        assert_eq!(
            find_meta(&meta, "bldimg"),
            Some("img@sha256:abc".to_string())
        );
    }

    #[test]
    fn find_meta_missing() {
        let meta = vec![entry("cliver", "v1")];
        assert_eq!(find_meta(&meta, "bldimg"), None);
    }

    #[test]
    fn find_meta_exact_key_not_prefix() {
        let meta = vec![entry("bldimg2", "wrong"), entry("bldimg", "right")];
        assert_eq!(find_meta(&meta, "bldimg"), Some("right".to_string()));
    }

    #[test]
    fn find_meta_empty() {
        let meta: Vec<ScMetaEntry> = Vec::new();
        assert_eq!(find_meta(&meta, "cliver"), None);
    }
}
