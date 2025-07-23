use cargo_metadata::{Metadata, MetadataCommand, Package};
use clap::Parser;
use itertools::Itertools;
use rustc_version::version;
use semver::Version;
use sha2::{Digest, Sha256};
use soroban_spec_tools::contract::Spec;
use std::{
    borrow::Cow,
    collections::HashSet,
    env,
    ffi::OsStr,
    fmt::Debug,
    fs, io::{self, Cursor},
    path::{self, Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
};
use stellar_xdr::curr::{Limits, Limited, ScMetaEntry, ScMetaV0, StringM, WriteXdr};

use crate::{commands::global, print::Print};

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
        .ok_or_else(|| Error::MetaArg("must be in the form 'key=value'".to_string()))?;

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
    #[error("deleting existing artifact: {0}")]
    DeletingArtifact(io::Error),
    #[error("copying wasm file: {0}")]
    CopyingWasmFile(io::Error),
    #[error("getting the current directory: {0}")]
    GettingCurrentDir(io::Error),
    #[error("retreiving CARGO_HOME: {0}")]
    CargoHome(io::Error),
    #[error("reading wasm file: {0}")]
    ReadingWasmFile(io::Error),
    #[error("writing wasm file: {0}")]
    WritingWasmFile(io::Error),
    #[error("invalid meta entry: {0}")]
    MetaArg(String),
    #[error("use rust 1.81 or 1.84+ to build contracts (got {0})")]
    RustVersion(String),
}

const WASM_TARGET: &str = "wasm32v1-none";
const WASM_TARGET_OLD: &str = "wasm32-unknown-unknown";
const META_CUSTOM_SECTION_NAME: &str = "contractmetav0";

impl Cmd {
    pub fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let print = Print::new(global_args.quiet);
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

        let wasm_target = get_wasm_target()?;

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
            cmd.arg(format!("--target={wasm_target}"));
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

            if let Some(rustflags) = make_rustflags_to_remap_absolute_paths(&print)? {
                cmd.env("CARGO_BUILD_RUSTFLAGS", rustflags);
            }

            let mut cmd_str_parts = Vec::<String>::new();
            cmd_str_parts.extend(cmd.get_envs().map(|(key, val)| {
                format!(
                    "{}={}",
                    key.to_string_lossy(),
                    shell_escape::escape(val.unwrap_or_default().to_string_lossy())
                )
            }));
            cmd_str_parts.push("cargo".to_string());
            cmd_str_parts.extend(
                cmd.get_args()
                    .map(OsStr::to_string_lossy)
                    .map(Cow::into_owned),
            );
            let cmd_str = cmd_str_parts.join(" ");

            if self.print_commands_only {
                println!("{cmd_str}");
            } else {
                print.infoln(cmd_str);
                let status = cmd.status().map_err(Error::CargoCmd)?;
                if !status.success() {
                    return Err(Error::Exit(status));
                }

                let file = format!("{}.wasm", p.name.replace('-', "_"));
                let target_file_path = Path::new(target_dir)
                    .join(&wasm_target)
                    .join(&self.profile)
                    .join(&file);

                self.handle_contract_metadata_args(&target_file_path)?;

                let final_path = if let Some(out_dir) = &self.out_dir {
                    fs::create_dir_all(out_dir).map_err(Error::CreatingOutDir)?;
                    let out_file_path = Path::new(out_dir).join(&file);
                    fs::copy(target_file_path, &out_file_path).map_err(Error::CopyingWasmFile)?;
                    out_file_path
                } else {
                    target_file_path
                };

                Self::print_build_summary(&print, &final_path)?;
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

        // get existing wasm bytes
        let mut wasm_bytes = fs::read(target_file_path).map_err(Error::ReadingWasmFile)?;


        // get existing meta entry
        let contract_spec = Spec::new(&wasm_bytes).unwrap();
        let mut existing_meta: Vec<ScMetaEntry> = contract_spec.meta;

        // collect meta args passed in
        for (k, v) in self.meta.clone() {
            let key: StringM = k
                .clone()
                .try_into()
                .map_err(|e| Error::MetaArg(format!("{k} is an invalid metadata key: {e}")))?;

            let val: StringM = v
                .clone()
                .try_into()
                .map_err(|e| Error::MetaArg(format!("{v} is an invalid metadata value: {e}")))?;
            let meta_entry = ScMetaEntry::ScMetaV0(ScMetaV0 { key, val });
            existing_meta.push(meta_entry);
        }

        // this puts them into a new section, but should probably put them into the existing meta section``
        let mut buf = Vec::new();
        let mut writer = Limited::new(std::io::Cursor::new(&mut buf), Limits::none());

        println!("existing_meta.leng() {}", existing_meta.len());

        (existing_meta.len() as u32).write_xdr(&mut writer).unwrap();

        for entry in existing_meta {
            entry.write_xdr(&mut writer).unwrap();
        }
        let xdr = writer.inner.into_inner();

        wasm_gen::write_custom_section(&mut wasm_bytes, META_CUSTOM_SECTION_NAME, &xdr);

        // Deleting .wasm file effectively unlinking it from /release/deps/.wasm preventing from overwrite
        // See https://github.com/stellar/stellar-cli/issues/1694#issuecomment-2709342205
        fs::remove_file(target_file_path).map_err(Error::DeletingArtifact)?;
        fs::write(target_file_path, wasm_bytes).map_err(Error::WritingWasmFile)
    }





    fn print_build_summary(print: &Print, target_file_path: &PathBuf) -> Result<(), Error> {
        print.infoln("Build Summary:");
        let rel_target_file_path = target_file_path
            .strip_prefix(env::current_dir().unwrap())
            .unwrap_or(target_file_path);
        print.blankln(format!("Wasm File: {}", rel_target_file_path.display()));

        let wasm_bytes = fs::read(target_file_path).map_err(Error::ReadingWasmFile)?;

        print.blankln(format!(
            "Wasm Hash: {}",
            hex::encode(Sha256::digest(&wasm_bytes))
        ));

        let parser = wasmparser::Parser::new(0);
        let export_names: Vec<&str> = parser
            .parse_all(&wasm_bytes)
            .filter_map(Result::ok)
            .filter_map(|payload| {
                if let wasmparser::Payload::ExportSection(exports) = payload {
                    Some(exports)
                } else {
                    None
                }
            })
            .flatten()
            .filter_map(Result::ok)
            .filter(|export| matches!(export.kind, wasmparser::ExternalKind::Func))
            .map(|export| export.name)
            .sorted()
            .collect();
        if export_names.is_empty() {
            print.blankln("Exported Functions: None found");
        } else {
            print.blankln(format!("Exported Functions: {} found", export_names.len()));
            for name in export_names {
                print.blankln(format!("  • {name}"));
            }
        }
        print.checkln("Build Complete");

        Ok(())
    }
}

/// Configure cargo/rustc to replace absolute paths in panic messages / debuginfo
/// with relative paths.
///
/// This is required for reproducible builds.
///
/// This works for paths to crates in the registry. The compiler already does
/// something similar for standard library paths and local paths. It may not
/// work for crates that come from other sources, including the standard library
/// compiled from source, though it may be possible to accomodate such cases in
/// the future.
///
/// This in theory breaks the ability of debuggers to find source code, but
/// since we are only targetting wasm, which is not typically run in a debugger,
/// and stellar-cli only compiles contracts in release mode, the impact is on
/// debugging is expected to be minimal.
///
/// This works by setting the `CARGO_BUILD_RUSTFLAGS` environment variable,
/// with appropriate `--remap-path-prefix` option. It preserves the values of an
/// existing `CARGO_BUILD_RUSTFLAGS` environment variable.
///
/// This must be done some via some variation of `RUSTFLAGS` and not as
/// arguments to `cargo rustc` because the latter only applies to the crate
/// directly being compiled, while `RUSTFLAGS` applies to all crates, including
/// dependencies.
///
/// `CARGO_BUILD_RUSTFLAGS` is an alias for the `build.rustflags` configuration
/// variable. Cargo automatically merges the contents of the environment variable
/// and the variables from config files; and `build.rustflags` has the lowest
/// priority of all the variations of rustflags that Cargo accepts. And because
/// we merge our values with an existing `CARGO_BUILD_RUSTFLAGS`,
/// our setting of this environment variable should not interfere with the
/// user's ability to set rustflags in any way they want, but it does mean
/// that if the user sets a higher-priority rustflags that our path remapping
/// will be ignored.
///
/// The major downside of using `CARGO_BUILD_RUSTFLAGS` is that it is whitespace
/// separated, which means we cannot support paths with spaces. If we encounter
/// such paths we will emit a warning. Spaces could be accomodated by using
/// `CARGO_ENCODED_RUSTFLAGS`, but that has high precedence over other rustflags,
/// so we could be interfering with the user's own use of rustflags. There is
/// no "encoded" variant of `CARGO_BUILD_RUSTFLAGS` at time of writing.
///
/// This assumes that paths are Unicode and that any existing `CARGO_BUILD_RUSTFLAGS`
/// variables are Unicode. Non-Unicode paths will fail to correctly perform the
/// the absolute path replacement. Non-Unicode `CARGO_BUILD_RUSTFLAGS` will result in the
/// existing rustflags being ignored, which is also the behavior of
/// Cargo itself.
fn make_rustflags_to_remap_absolute_paths(print: &Print) -> Result<Option<String>, Error> {
    let cargo_home = home::cargo_home().map_err(Error::CargoHome)?;

    if format!("{}", cargo_home.display())
        .find(|c: char| c.is_whitespace())
        .is_some()
    {
        print.warnln("Cargo home directory contains whitespace. Dependency paths will not be remapped; builds may not be reproducible.");
        return Ok(None);
    }

    if env::var("RUSTFLAGS").is_ok() {
        print.warnln("`RUSTFLAGS` set. Dependency paths will not be remapped; builds may not be reproducible.");
        return Ok(None);
    }

    if env::var("CARGO_ENCODED_RUSTFLAGS").is_ok() {
        print.warnln("`CARGO_ENCODED_RUSTFLAGS` set. Dependency paths will not be remapped; builds may not be reproducible.");
        return Ok(None);
    }

    let target = get_wasm_target()?;
    let env_var_name = format!("TARGET_{target}_RUSTFLAGS");

    if env::var(env_var_name.clone()).is_ok() {
        print.warnln(format!("`{env_var_name}` set. Dependency paths will not be remapped; builds may not be reproducible."));
        return Ok(None);
    }

    let registry_prefix = cargo_home.join("registry").join("src");
    let registry_prefix_str = registry_prefix.display().to_string();
    #[cfg(windows)]
    let registry_prefix_str = registry_prefix_str.replace('\\', "/");
    let new_rustflag = format!("--remap-path-prefix={registry_prefix_str}=");

    let mut rustflags = get_rustflags().unwrap_or_default();
    rustflags.push(new_rustflag);

    let rustflags = rustflags.join(" ");

    Ok(Some(rustflags))
}

/// Get any existing `CARGO_BUILD_RUSTFLAGS`, split on whitespace.
///
/// This conveniently ignores non-Unicode values, as does Cargo.
fn get_rustflags() -> Option<Vec<String>> {
    if let Ok(a) = env::var("CARGO_BUILD_RUSTFLAGS") {
        let args = a
            .split_whitespace()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        return Some(args.collect());
    }

    None
}

fn get_wasm_target() -> Result<String, Error> {
    let Ok(current_version) = version() else {
        return Ok(WASM_TARGET.into());
    };

    let v184 = Version::parse("1.84.0").unwrap();
    let v182 = Version::parse("1.82.0").unwrap();

    if current_version >= v182 && current_version < v184 {
        return Err(Error::RustVersion(current_version.to_string()));
    }

    if current_version < v184 {
        Ok(WASM_TARGET_OLD.into())
    } else {
        Ok(WASM_TARGET.into())
    }
}
