use clap::Parser;
use itertools::Itertools;
use std::{
    collections::HashSet,
    ffi::OsStr,
    fmt::Debug,
    process::{Command, Stdio},
};

use cargo_metadata::{Metadata, MetadataCommand, Package};

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    /// Path to Cargo.toml
    #[arg(long)]
    pub manifest_path: Option<std::path::PathBuf>,
    /// Package to build
    #[arg(long)]
    pub package: Option<String>,
    /// Build with the specified profile
    #[arg(long, default_value = "release")]
    pub profile: String,
    /// Build with the list of features activated, space or comma separated
    #[arg(
        long,
        conflicts_with = "all_features",
        conflicts_with = "no_default_features"
    )]
    pub features: Option<String>,
    /// Build with the all features activated, space or comma separated
    #[arg(
        long,
        conflicts_with = "features",
        conflicts_with = "no_default_features"
    )]
    pub all_features: bool,
    /// Build with the default feature not activated
    #[arg(long, conflicts_with = "features", conflicts_with = "all_features")]
    pub no_default_features: bool,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Metadata(#[from] cargo_metadata::Error),
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        let packages = self.packages()?;

        for p in packages {
            let mut cmd = Command::new("cargo");
            cmd.stdout(Stdio::piped());
            cmd.arg("rustc");
            // TODO: Convert the manifest path into a relative path if possible,
            // to improve the console output.
            cmd.arg(format!("--manifest-path={}", p.manifest_path));
            cmd.arg("--crate-type=cdylib");
            cmd.arg("--target=wasm32-unknown-unknown");
            cmd.arg(format!("--package={}", p.name));
            cmd.arg(format!("--profile={}", self.profile));
            if self.all_features {
                cmd.arg("--all-features");
            }
            if self.no_default_features {
                cmd.arg("--no-default-features");
            }
            if let Some(features) = self.features() {
                let requested: HashSet<String> = features.iter().cloned().collect();
                let available = p.features.iter().map(|f| f.0).cloned().collect();
                let activate = requested.intersection(&available).join(",");
                if !activate.is_empty() {
                    cmd.arg(format!("--features={activate}"));
                }
            }
            eprintln!(
                "cargo {}",
                cmd.get_args().map(OsStr::to_string_lossy).join(" ")
            );
            let _ = cmd.status();
        }

        Ok(())
    }

    fn features(&self) -> Option<Vec<String>> {
        self.features
            .as_ref()
            .map(|f| f.split(&[',', ' ']).map(String::from).collect())
    }

    fn packages(&self) -> Result<Vec<Package>, cargo_metadata::Error> {
        let metadata = self.metadata()?;
        let iter = metadata
            .packages
            .iter()
            .filter(|p| self.package.is_none() || Some(&p.name) == self.package.as_ref())
            .filter(|p| {
                p.targets
                    .iter()
                    .any(|t| t.crate_types.iter().any(|c| c == "cdylib"))
            })
            .cloned();
        Ok(iter.collect())
    }

    fn metadata(&self) -> Result<Metadata, cargo_metadata::Error> {
        let mut cmd = MetadataCommand::new();
        cmd.no_deps();
        if let Some(manifest_path) = &self.manifest_path {
            cmd.manifest_path(manifest_path);
        }
        // Do not configure features on the metadata command, because we are
        // only collecting non-dependency metadata, features have no impact on
        // the output.
        cmd.exec()
    }
}
