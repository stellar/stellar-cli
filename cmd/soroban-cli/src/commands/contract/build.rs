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

use cargo_metadata::{Metadata, MetadataCommand, Package};

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
    #[error("retreiving CARGO_HOME: {0}")]
    CargoHome(io::Error),
}

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
            set_env_to_remap_absolute_paths(&mut cmd)?;
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
fn set_env_to_remap_absolute_paths(cmd: &mut Command) -> Result<(), Error> {
    let cargo_home = home::cargo_home().map_err(Error::CargoHome)?;
    let cargo_home = format!("{}", cargo_home.display());

    if cargo_home.find(|c: char| c.is_whitespace()).is_some() {
        eprintln!(
            "⚠  Warning: Cargo home directory contains whitespace. \
                   Dependency paths will not be remapped; builds may not be reproducible."
        );
        return Ok(());
    }

    if env::var("RUSTFLAGS").is_ok() {
        eprintln!(
            "⚠  Warning: `RUSTFLAGS` set. \
                   Dependency paths will not be remapped; builds may not be reproducible."
        );
        return Ok(());
    }

    if env::var("CARGO_ENCODED_RUSTFLAGS").is_ok() {
        eprintln!(
            "⚠  Warning: `CARGO_ENCODED_RUSTFLAGS` set. \
                   Dependency paths will not be remapped; builds may not be reproducible."
        );
        return Ok(());
    }

    if env::var("TARGET_wasm32-unknown-unknown_RUSTFLAGS").is_ok() {
        eprintln!(
            "⚠  Warning: `TARGET_wasm32-unknown-unknown_RUSTFLAGS` set. \
                   Dependency paths will not be remapped; builds may not be reproducible."
        );
        return Ok(());
    }

    let registry_prefix = format!("{cargo_home}/registry/src/");
    let new_rustflag = format!("--remap-path-prefix={registry_prefix}=");

    let mut rustflags = get_rustflags().unwrap_or_default();
    rustflags.push(new_rustflag);

    let rustflags = rustflags.join(" ");
    cmd.env("CARGO_BUILD_RUSTFLAGS", rustflags);

    Ok(())
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
