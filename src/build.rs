use std::path::PathBuf;
use std::{ffi::OsStr, fmt::Debug, process::Command};

use clap::Parser;
use clap_cargo_extra::ClapCargo;

#[cfg(feature = "binaryen")]
mod opt;

#[derive(Parser, Debug, Clone, Default)]
pub struct Cmd {
    #[clap(flatten)]
    pub cargo: ClapCargo,

    /// output for optimized wasm, default [name]_opt.wasm
    #[clap(long)]
    pub optimized_output: Option<PathBuf>,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Failed Command:\n{0}")]
    Build(String),

    #[cfg(feature = "binaryen")]
    #[error("Error with cargo {0}")]
    Cargo(String),
}

fn cmd_str(cmd: &Command) -> String {
    format!(
        "{} {}",
        cmd.get_program().to_string_lossy(),
        cmd.get_args()
            .map(OsStr::to_string_lossy)
            .collect::<Vec<_>>()
            .join(" ")
    )
}

fn build_err(cmd: &Command) -> Error {
    Error::Build(cmd_str(cmd))
}

impl Cmd {
    #[allow(dead_code, clippy::must_use_candidate)]
    pub fn optimized() -> Self {
        let mut cmd = Self::default();
        std::env::remove_var("CARGO");
        cmd.cargo.cargo_bin.channel = "nightly".to_string();
        cmd.cargo.optimize = true;
        cmd.cargo.release = true;
        cmd.cargo.target = Some("wasm32-unknown-unknown".to_string());
        cmd
    }

    /// Build the current package or the workspace
    ///
    /// # Errors
    ///
    /// Could fail to build when executing the command
    ///
    pub fn run(&self) -> Result<(), Error> {
        let mut cargo = self.cargo.clone();
        cargo.target = Some(
            cargo
                .target
                .unwrap_or_else(|| "wasm32-unknown-unknown".to_string()),
        );
        let mut cmd = cargo.build_cmd();
        let status = cmd.status().map_err(|_| build_err(&cmd))?;
        if status.success() {
            #[cfg(feature = "binaryen")]
            for p in cargo.current_packages().map_err(|_| build_err(&cmd))? {
                let t = &p.targets[0];
                if self.should_rebuild(t).unwrap_or(true) {
                    b::optimize(&self.orig_bin(t)?, &self.output_bin(t)?);
                }
            }
            Ok(())
        } else {
            Err(build_err(&cmd))
        }
    }
}
