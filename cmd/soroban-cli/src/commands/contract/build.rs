use clap::Parser;
use itertools::Itertools;
use std::{
    collections::HashSet,
    env,
    ffi::OsStr,
    fmt::Debug,
    fs, io,
    path::Path,
    process::{Command, ExitStatus, Stdio},
};

use cargo_metadata::{Metadata, MetadataCommand, Package};

/// Build a contract from source
///
/// Builds all crates that are referenced by the cargo manifest (Cargo.toml)
/// that have cdylib as their crate-type. Crates are built for the wasm32
/// target. Unless configured otherwise, crates are built with their default
/// features and with their release profile.
///
/// To view the commands that will be executed, without executing them, use the
/// --print-commands-only option.
#[derive(Parser, Debug, Clone)]
pub struct Cmd {
    /// Path to Cargo.toml
    #[arg(long, default_value = "Cargo.toml")]
    pub manifest_path: std::path::PathBuf,
    /// Package to build
    ///
    /// If omitted, all packages that build for crate-type cdylib are built.
    #[arg(long)]
    pub package: Option<String>,
    /// Build with the specified profile
    #[arg(long, default_value = "release")]
    pub profile: String,
    /// Build with the list of features activated, space or comma separated
    #[arg(long, help_heading = "Features")]
    pub features: Option<String>,
    /// Build with the all features activated
    #[arg(
        long,
        conflicts_with = "features",
        conflicts_with = "no_default_features",
        help_heading = "Features"
    )]
    pub all_features: bool,
    /// Build with the default feature not activated
    #[arg(long, help_heading = "Features")]
    pub no_default_features: bool,
    /// Directory to copy wasm files to
    ///
    /// If provided, wasm files can be found in the cargo target directory, and
    /// the specified directory.
    ///
    /// If ommitted, wasm files are written only to the cargo target directory.
    #[arg(long)]
    pub out_dir: Option<std::path::PathBuf>,
    /// Print commands to build without executing them
    #[arg(long, conflicts_with = "out_dir", help_heading = "Other")]
    pub print_commands_only: bool,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Metadata(#[from] cargo_metadata::Error),
    #[error(transparent)]
    CargoCmd(io::Error),
    #[error("exit status {0}")]
    Exit(ExitStatus),
    #[error("package {package} not found")]
    PackageNotFound { package: String },
    #[error("creating out directory: {0}")]
    CreatingOutDir(io::Error),
    #[error("copying wasm file: {0}")]
    CopyingWasmFile(io::Error),
    #[error("getting the current directory: {0}")]
    GettingCurrentDir(io::Error),
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        let working_dir = env::current_dir().map_err(Error::GettingCurrentDir)?;

        let metadata = self.metadata()?;
        let packages = self.packages(&metadata);
        let target_dir = &metadata.target_directory;

        if let Some(package) = &self.package {
            if packages.is_empty() {
                return Err(Error::PackageNotFound {
                    package: package.clone(),
                });
            }
        }

        for p in packages {
            let mut cmd = Command::new("cargo");
            cmd.stdout(Stdio::piped());
            cmd.arg("rustc");
            let manifest_path = pathdiff::diff_paths(&p.manifest_path, &working_dir)
                .unwrap_or(p.manifest_path.clone().into());
            cmd.arg(format!(
                "--manifest-path={}",
                manifest_path.to_string_lossy()
            ));
            cmd.arg("--crate-type=cdylib");
            cmd.arg("--target=wasm32-unknown-unknown");
            if self.profile == "release" {
                cmd.arg("--release");
            } else {
                cmd.arg(format!("--profile={}", self.profile));
            }
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
            let cmd_str = format!(
                "cargo {}",
                cmd.get_args().map(OsStr::to_string_lossy).join(" ")
            );

            if self.print_commands_only {
                println!("{cmd_str}");
            } else {
                eprintln!("{cmd_str}");
                let status = cmd.status().map_err(Error::CargoCmd)?;
                if !status.success() {
                    return Err(Error::Exit(status));
                }

                if let Some(out_dir) = &self.out_dir {
                    fs::create_dir_all(out_dir).map_err(Error::CreatingOutDir)?;

                    let file = format!("{}.wasm", p.name.replace('-', "_"));
                    let target_file_path = Path::new(target_dir)
                        .join("wasm32-unknown-unknown")
                        .join(&self.profile)
                        .join(&file);
                    let out_file_path = Path::new(out_dir).join(&file);
                    fs::copy(target_file_path, out_file_path).map_err(Error::CopyingWasmFile)?;
                }
            }
        }

        Ok(())
    }

    fn features(&self) -> Option<Vec<String>> {
        self.features
            .as_ref()
            .map(|f| f.split(&[',', ' ']).map(String::from).collect())
    }

    fn packages(&self, metadata: &Metadata) -> Vec<Package> {
        metadata
            .packages
            .iter()
            .filter(|p|
                // Filter by the package name if one is provided.
                self.package.is_none() || Some(&p.name) == self.package.as_ref())
            .filter(|p| {
                // Filter crates by those that build to cdylib (wasm), unless a
                // package is provided.
                self.package.is_some()
                    || p.targets
                        .iter()
                        .any(|t| t.crate_types.iter().any(|c| c == "cdylib"))
            })
            .cloned()
            .collect()
    }

    fn metadata(&self) -> Result<Metadata, cargo_metadata::Error> {
        let mut cmd = MetadataCommand::new();
        cmd.no_deps();
        cmd.manifest_path(&self.manifest_path);
        // Do not configure features on the metadata command, because we are
        // only collecting non-dependency metadata, features have no impact on
        // the output.
        cmd.exec()
    }
}
