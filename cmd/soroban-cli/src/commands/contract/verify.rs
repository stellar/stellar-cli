use std::fs;
use std::path::PathBuf;

use clap::Parser;
use sha2::{Digest, Sha256};
use soroban_spec_tools::contract::{self, Spec};
use stellar_xdr::curr::{ScMetaEntry, ScMetaV0};

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
    #[error("verification failed: rebuilt wasm does not match original (sha256 {expected})")]
    Mismatch { expected: String },
    #[error("reading rebuilt wasm: {0}")]
    ReadingRebuilt(std::io::Error),
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
        let cliver = find_meta(&spec, "cliver").ok_or(Error::MissingMeta("cliver"))?;
        let bldimg = match &self.docker {
            Some(image) => image.clone(),
            None => find_meta(&spec, "bldimg").ok_or(Error::MissingMeta("bldimg"))?,
        };

        let running_cliver = version::one_line();
        if cliver != running_cliver {
            return Err(Error::CliVersionMismatch {
                expected: cliver,
                actual: running_cliver,
            });
        }
        print.infoln(format!("CLI version matches: {running_cliver}"));

        print.infoln(format!("Rebuilding with docker image {bldimg}..."));
        let build_cmd = build::Cmd {
            manifest_path: Some(self.source.join("Cargo.toml")),
            docker: Some(bldimg),
            ..build::Cmd::default()
        };
        let built = build_cmd.run(global_args).await?;

        let mut hashes: Vec<(String, String)> = Vec::with_capacity(built.len());
        let mut matched: Option<String> = None;
        for c in &built {
            let bytes = fs::read(&c.path).map_err(Error::ReadingRebuilt)?;
            let hash = hex::encode(Sha256::digest(&bytes));
            if hash == original_hash {
                matched = Some(c.name.clone());
            }
            hashes.push((c.name.clone(), hash));
        }

        if let Some(name) = matched {
            eprintln!(
                "✅ Verified: rebuilt wasm matches the original (sha256 {original_hash}) — {name}"
            );
            Ok(())
        } else {
            eprintln!("⚠ Verification failed: rebuilt wasm does not match original.");
            eprintln!("   Built artifacts:");
            for (name, hash) in &hashes {
                eprintln!("     {name}  {hash}");
            }
            Err(Error::Mismatch {
                expected: original_hash,
            })
        }
    }
}

fn find_meta(spec: &Spec, key: &str) -> Option<String> {
    spec.meta.iter().find_map(|meta_entry| {
        let ScMetaEntry::ScMetaV0(ScMetaV0 { key: k, val }) = meta_entry;
        if k.to_string() == key {
            Some(val.to_string())
        } else {
            None
        }
    })
}
