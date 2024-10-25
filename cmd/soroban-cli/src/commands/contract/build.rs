use cargo_metadata::{Metadata, MetadataCommand, Package};
use clap::Parser;
use itertools::Itertools;
use std::{
    collections::HashSet,
    env,
    ffi::OsStr,
    fmt::Debug,
    fs, io,
    path::{self, Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
};
use stellar_xdr::curr::{Limits, ScMetaEntry, ScMetaV0, StringM, WriteXdr};

/// Build a contract from source
///
/// Builds all crates that are referenced by the cargo manifest (Cargo.toml)
/// that have cdylib as their crate-type. Crates are built for the wasm32
/// target. Unless configured otherwise, crates are built with their default
/// features and with their release profile.
///
/// In workspaces builds all crates unless a package name is specified, or the
/// command is executed from the sub-directory of a workspace crate.
///
/// To view the commands that will be executed, without executing them, use the
/// --print-commands-only option.
#[derive(Parser, Debug, Clone)]
pub struct Cmd {
    /// Path to Cargo.toml
    #[arg(long)]
    pub manifest_path: Option<std::path::PathBuf>,
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
    /// Add key-value to contract meta (adds the meta to the `contractmetav0` custom section)
    #[arg(long, num_args=1, value_parser=parse_meta_arg, action=clap::ArgAction::Append, help_heading = "Metadata")]
    pub meta: Vec<(String, String)>,
}

fn parse_meta_arg(s: &str) -> Result<(String, String), Error> {
    let parts = s.splitn(2, '=');

    let (key, value) = parts
        .map(str::trim)
        .next_tuple()
        .ok_or_else(|| Error::MetadataArg("must be in the form 'key=value'".to_string()))?;

    Ok((key.to_string(), value.to_string()))
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
    #[error("finding absolute path of Cargo.toml: {0}")]
    AbsolutePath(io::Error),
    #[error("creating out directory: {0}")]
    CreatingOutDir(io::Error),
    #[error("copying wasm file: {0}")]
    CopyingWasmFile(io::Error),
    #[error("getting the current directory: {0}")]
    GettingCurrentDir(io::Error),
    #[error("reading wasm file: {0}")]
    ReadingWasmFile(io::Error),
    #[error("writing wasm file: {0}")]
    WritingWasmFile(io::Error),
    #[error("invalid metadata entry: {0}")]
    MetadataArg(String),
}

const WASM_TARGET: &str = "wasm32-unknown-unknown";
const METADATA_CUSTOM_SECTION_NAME: &str = "contractmetav0";

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        let working_dir = env::current_dir().map_err(Error::GettingCurrentDir)?;

        let metadata = self.metadata()?;
        let packages = self.packages(&metadata)?;
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
            cmd.arg(format!("--target={WASM_TARGET}"));
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

                let file = format!("{}.wasm", p.name.replace('-', "_"));
                let target_file_path = Path::new(target_dir)
                    .join(WASM_TARGET)
                    .join(&self.profile)
                    .join(&file);

                self.handle_contract_metadata_args(&target_file_path)?;

                if let Some(out_dir) = &self.out_dir {
                    fs::create_dir_all(out_dir).map_err(Error::CreatingOutDir)?;
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

    fn packages(&self, metadata: &Metadata) -> Result<Vec<Package>, Error> {
        // Filter by the package name if one is provided, or by the package that
        // matches the manifest path if the manifest path matches a specific
        // package.
        let name = if let Some(name) = self.package.clone() {
            Some(name)
        } else {
            // When matching a package based on the manifest path, match against the
            // absolute path because the paths in the metadata are absolute. Match
            // against a manifest in the current working directory if no manifest is
            // specified.
            let manifest_path = path::absolute(
                self.manifest_path
                    .clone()
                    .unwrap_or(PathBuf::from("Cargo.toml")),
            )
            .map_err(Error::AbsolutePath)?;
            metadata
                .packages
                .iter()
                .find(|p| p.manifest_path == manifest_path)
                .map(|p| p.name.clone())
        };

        let packages = metadata
            .packages
            .iter()
            .filter(|p|
                // Filter by the package name if one is selected based on the above logic.
                if let Some(name) = &name {
                    &p.name == name
                } else {
                    // Otherwise filter crates that are default members of the
                    // workspace and that build to cdylib (wasm).
                    metadata.workspace_default_members.contains(&p.id)
                        && p.targets
                            .iter()
                            .any(|t| t.crate_types.iter().any(|c| c == "cdylib"))
                }
            )
            .cloned()
            .collect();

        Ok(packages)
    }

    fn metadata(&self) -> Result<Metadata, cargo_metadata::Error> {
        let mut cmd = MetadataCommand::new();
        cmd.no_deps();
        // Set the manifest path if one is provided, otherwise rely on the cargo
        // commands default behavior of finding the nearest Cargo.toml in the
        // current directory, or the parent directories above it.
        if let Some(manifest_path) = &self.manifest_path {
            cmd.manifest_path(manifest_path);
        }
        // Do not configure features on the metadata command, because we are
        // only collecting non-dependency metadata, features have no impact on
        // the output.
        cmd.exec()
    }

    fn handle_contract_metadata_args(&self, target_file_path: &PathBuf) -> Result<(), Error> {
        if self.meta.is_empty() {
            return Ok(());
        }

        let mut wasm_bytes = fs::read(target_file_path).map_err(Error::ReadingWasmFile)?;

        for (k, v) in self.meta.clone() {
            let key: StringM = k
                .clone()
                .try_into()
                .map_err(|e| Error::MetadataArg(format!("{k} is an invalid metadata key: {e}")))?;

            let val: StringM = v.clone().try_into().map_err(|e| {
                Error::MetadataArg(format!("{v} is an invalid metadata value: {e}"))
            })?;
            let meta_entry = ScMetaEntry::ScMetaV0(ScMetaV0 { key, val });
            let xdr: Vec<u8> = meta_entry
                .to_xdr(Limits::none())
                .map_err(|e| Error::MetadataArg(format!("failed to encode metadata entry: {e}")))?;

            wasm_gen::write_custom_section(&mut wasm_bytes, METADATA_CUSTOM_SECTION_NAME, &xdr);
        }

        Ok(fs::write(target_file_path, wasm_bytes).map_err(Error::WritingWasmFile)?)
    }
}
