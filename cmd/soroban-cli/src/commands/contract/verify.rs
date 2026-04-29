use std::fs;
use std::path::PathBuf;

use cargo_metadata::MetadataCommand;
use clap::Parser;
use sha2::{Digest, Sha256};
use soroban_spec_tools::contract::{self, Spec};
use stellar_xdr::curr::{ScMetaEntry, ScMetaV0};

use super::build;
use super::info::shared::{self, fetch, Contract, Fetched};
use crate::commands::container::shared::Args as ContainerArgs;
use crate::commands::{global, version};
use crate::print::Print;

/// Verify a wasm by rebuilding it inside the container image recorded in its metadata.
///
/// Succeeds only if the rebuilt artifact is byte-identical to the input.
/// User is responsible for checking out the matching commit before running.
#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub common: shared::Args,

    /// Path to Cargo.toml of the source to rebuild. Defaults to the nearest
    /// Cargo.toml in the current directory or its parents.
    #[arg(long)]
    pub manifest_path: Option<PathBuf>,

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
    #[error("required '{0}' meta entry not found in contract; rebuild the wasm with `stellar contract build --backend container` to make it verifiable")]
    MissingMeta(&'static str),
    #[error("stellar-cli version mismatch: contract was built with '{expected}', running stellar-cli is '{actual}'. Install the matching CLI version and re-run.")]
    CliVersionMismatch { expected: String, actual: String },
    #[error("verification failed: rebuilt {name} ({actual}) does not match original ({expected})")]
    Mismatch {
        name: String,
        expected: String,
        actual: String,
    },
    #[error("reading rebuilt wasm: {0}")]
    ReadingRebuilt(std::io::Error),
    #[error("expected source to produce exactly one cdylib contract, found {found:?}; verify only supports a single contract per invocation")]
    ExpectedSingleContract { found: Vec<String> },
    #[error("reading cargo metadata: {0}")]
    Metadata(#[from] cargo_metadata::Error),
    #[error("resolving source path: {0}")]
    AbsolutePath(std::io::Error),
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
        let spec = Spec::new(&wasm_bytes)?;
        let cliver = find_meta(&spec.meta, "cliver").ok_or(Error::MissingMeta("cliver"))?;
        let bldimg = find_meta(&spec.meta, "bldimg").ok_or(Error::MissingMeta("bldimg"))?;
        let rsver = find_meta(&spec.meta, "rsver").ok_or(Error::MissingMeta("rsver"))?;
        print.blankln(format!("Original wasm hash: {original_hash}"));
        print.blankln(format!("stellar-cli version: {cliver}"));
        print.blankln(format!("rust version: {rsver}"));
        print.blankln(format!("Container image: {bldimg}"));

        let running = version::one_line();
        if cliver != running {
            return Err(Error::CliVersionMismatch {
                expected: cliver,
                actual: running,
            });
        }

        // Verify takes a single wasm input, so the source must produce exactly
        // one cdylib contract. Detect this up-front via cargo metadata so we
        // don't waste a build cycle on a workspace with multiple contracts.
        let cdylibs = single_cdylib_or_workspace_cdylibs(self.manifest_path.as_deref())?;
        if cdylibs.len() != 1 {
            return Err(Error::ExpectedSingleContract { found: cdylibs });
        }

        let build_cmd = build::Cmd {
            manifest_path: self.manifest_path.clone(),
            backend: build::Backend::Container { image: bldimg },
            container_args: self.container_args.clone(),
            rustup_toolchain: Some(rsver),
            ..build::Cmd::default()
        };
        let built = build_cmd.run(global_args).await?;
        let c = match built.as_slice() {
            [c] => c,
            other => {
                return Err(Error::ExpectedSingleContract {
                    found: other.iter().map(|c| c.name.clone()).collect(),
                });
            }
        };

        let bytes = fs::read(&c.path).map_err(Error::ReadingRebuilt)?;
        let hash = hex::encode(Sha256::digest(&bytes));
        if hash == original_hash {
            print.checkln(format!(
                "Verified: rebuilt {} wasm matches {original_hash}",
                c.name
            ));
            Ok(())
        } else {
            Err(Error::Mismatch {
                name: c.name.clone(),
                expected: original_hash,
                actual: hash,
            })
        }
    }
}

/// Mirror what `build::Cmd::packages` selects: if `manifest_path` points at a
/// specific package, return that package's name iff it is a cdylib; otherwise
/// (workspace root, or no manifest given) return the names of all
/// workspace-member cdylibs.
fn single_cdylib_or_workspace_cdylibs(
    manifest_path: Option<&std::path::Path>,
) -> Result<Vec<String>, Error> {
    let mut cmd = MetadataCommand::new();
    cmd.no_deps();
    if let Some(p) = manifest_path {
        cmd.manifest_path(p);
    }
    let metadata = cmd.exec()?;
    let manifest_abs = match manifest_path {
        Some(p) => Some(std::path::absolute(p).map_err(Error::AbsolutePath)?),
        None => None,
    };
    let is_cdylib = |p: &cargo_metadata::Package| {
        p.targets
            .iter()
            .any(|t| t.crate_types.iter().any(|c| c == "cdylib"))
    };
    let specific = manifest_abs
        .as_ref()
        .and_then(|abs| metadata.packages.iter().find(|p| p.manifest_path == *abs));
    Ok(match specific {
        Some(p) if is_cdylib(p) => vec![p.name.clone()],
        Some(_) => vec![],
        None => metadata
            .packages
            .iter()
            .filter(|p| metadata.workspace_members.contains(&p.id))
            .filter(|p| is_cdylib(p))
            .map(|p| p.name.clone())
            .collect(),
    })
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
