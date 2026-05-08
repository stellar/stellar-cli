use std::fs;
use std::path::PathBuf;

use clap::Parser;
use sha2::{Digest, Sha256};
use soroban_spec_tools::contract::{self, Spec};
use stellar_xdr::curr::{ScMetaEntry, ScMetaV0};

use super::build;
use super::info::shared::{self, fetch, Contract, Fetched};
use crate::commands::container::shared::Args as ContainerArgs;
use crate::commands::global;
use crate::print::Print;

/// Verify a wasm by rebuilding it and comparing bytes.
///
/// The wasm's `bldimg` meta entry identifies the container image used for
/// the original build; verify pulls that image and rebuilds inside it. The
/// image bundles its own rust toolchain, and the in-container shim exports
/// `RUSTUP_TOOLCHAIN` to the image's default before invoking cargo so an
/// in-source `rust-toolchain.toml` can't silently switch versions. All
/// cdylib contracts in the workspace are rebuilt; verification succeeds
/// if any rebuilt artifact is byte-identical to the input. The user is
/// responsible for checking out the matching commit before running.
#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub common: shared::Args,

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
    #[error("required '{0}' meta entry not found in contract; rebuild the wasm with `stellar contract build --backend docker` to make it verifiable")]
    MissingMeta(&'static str),
    #[error("verification failed: none of the rebuilt artifacts ({}) match original ({expected})", produced.iter().map(|(n, h)| format!("{n}={h}")).collect::<Vec<_>>().join(", "))]
    Mismatch {
        expected: String,
        produced: Vec<(String, String)>,
    },
    #[error("reading rebuilt wasm: {0}")]
    ReadingRebuilt(std::io::Error),
}

impl Cmd {
    #[allow(clippy::too_many_lines)]
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let print = Print::new(global_args.quiet);

        let Fetched { contract, .. } = fetch(&self.common, &print).await?;
        let wasm_bytes = match contract {
            Contract::Wasm { wasm_bytes } => wasm_bytes,
            Contract::StellarAssetContract => return Err(Error::StellarAssetContract),
        };
        let original_hash = hex::encode(Sha256::digest(&wasm_bytes));
        let spec = Spec::new(&wasm_bytes)?;
        let bldimg = find_meta(&spec.meta, "bldimg").ok_or(Error::MissingMeta("bldimg"))?;
        // `bldopt` is a repeating shell-form flag set. Apply the SEP
        // baseline defaults, then overlay each recorded flag.
        let bldopts = find_meta_all(&spec.meta, "bldopt");
        let mut manifest_path_str: String = "Cargo.toml".into();
        let mut package: Option<String> = None;
        let mut profile: String = "release".into();
        let mut optimize = true;
        let mut features: Option<String> = None;
        let mut all_features = false;
        let mut no_default_features = false;
        for opt in &bldopts {
            if let Some(v) = opt.strip_prefix("--manifest-path=") {
                manifest_path_str = v.to_string();
            } else if let Some(v) = opt.strip_prefix("--package=") {
                package = Some(v.to_string());
            } else if let Some(v) = opt.strip_prefix("--profile=") {
                profile = v.to_string();
            } else if opt == "--no-optimize" {
                optimize = false;
            } else if let Some(v) = opt.strip_prefix("--features=") {
                features = Some(v.to_string());
            } else if opt == "--all-features" {
                all_features = true;
            } else if opt == "--no-default-features" {
                no_default_features = true;
            } else {
                print.warnln(format!(
                    "ignoring unrecognized bldopt flag: {opt}. The rebuild may not match the original."
                ));
            }
        }

        print.blankln(format!("Original wasm hash: {original_hash}"));
        print.blankln(format!("Docker image: {bldimg}"));
        for opt in &bldopts {
            print.blankln(format!("Build flag: {opt}"));
        }

        // Resolve the manifest path relative to the cwd's git top-level so
        // verify works from anywhere inside the checkout.
        let manifest_path = {
            let p = PathBuf::from(&manifest_path_str);
            if p.is_absolute() {
                Some(p)
            } else if let Some(root) = git_top_level() {
                Some(root.join(p))
            } else {
                Some(p)
            }
        };

        let build_cmd = build::Cmd {
            manifest_path,
            package,
            profile,
            features,
            all_features,
            no_default_features,
            backend: build::Backend::Docker { image: bldimg },
            container_args: self.container_args.clone(),
            build_args: build::BuildArgs {
                optimize,
                ..build::BuildArgs::default()
            },
            ..build::Cmd::default()
        };
        let built = build_cmd.run(global_args).await?;

        // Hash every rebuilt artifact and find one that matches.
        let mut produced = Vec::with_capacity(built.len());
        let mut matched = None;
        for c in &built {
            let bytes = fs::read(&c.path).map_err(Error::ReadingRebuilt)?;
            let hash = hex::encode(Sha256::digest(&bytes));
            if matched.is_none() && hash == original_hash {
                matched = Some(c.name.clone());
            }
            produced.push((c.name.clone(), hash));
        }

        // For multi-contract workspaces, separate the per-contract build
        // output from the final verdict with a blank line.
        if built.len() > 1 {
            eprintln!();
        }
        if let Some(name) = matched {
            print.checkln(format!(
                "Verified: rebuilt {name} wasm matches {original_hash}"
            ));
            Ok(())
        } else {
            Err(Error::Mismatch {
                expected: original_hash,
                produced,
            })
        }
    }
}

fn git_top_level() -> Option<PathBuf> {
    let out = std::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| PathBuf::from(String::from_utf8_lossy(&out.stdout).trim()))
}

fn find_meta(meta: &[ScMetaEntry], key: &str) -> Option<String> {
    meta.iter().find_map(|entry| {
        let ScMetaEntry::ScMetaV0(ScMetaV0 { key: k, val }) = entry;
        (k.to_string() == key).then(|| val.to_string())
    })
}

fn find_meta_all(meta: &[ScMetaEntry], key: &str) -> Vec<String> {
    meta.iter()
        .filter_map(|entry| {
            let ScMetaEntry::ScMetaV0(ScMetaV0 { key: k, val }) = entry;
            (k.to_string() == key).then(|| val.to_string())
        })
        .collect()
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
