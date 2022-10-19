use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::{ffi::OsStr, fmt::Debug, process::Command};

use cargo_metadata::Target;
use clap::Parser;
use clap_cargo_extra::ClapCargo;
use filetime::FileTime;

#[derive(Parser, Debug, Clone)]
pub struct Cmd {
    #[clap(flatten)]
    cargo: ClapCargo,

    /// output for optimized wasm, default [name]_opt.wasm
    #[clap(long)]
    optimized_output: Option<PathBuf>,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Failed Command:\n{0}")]
    Build(String),

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
    pub fn run(&self) -> Result<(), Error> {
        let mut cargo = self.cargo.clone();
        cargo.target = Some(
            cargo
                .target
                .unwrap_or_else(|| "wasm32-unknown-unknown".to_string()),
        );
        let mut cmd = cargo.build_cmd();
        let status = cmd.status().map_err(|_| build_err(&cmd))?;
        if !status.success() {
            Err(build_err(&cmd))
        } else {
            for p in cargo.current_packages().map_err(|_| build_err(&cmd))? {
                let t = &p.targets[0];
                if self.should_rebuild(t).unwrap_or(true) {
                    optimize(&self.orig_bin(t)?, &self.output_bin(t)?);
                }
            }
            Ok(())
        }
    }

    pub fn bin_name(target: &Target) -> String {
        format!("{}.wasm", target.name.replace('-', "_"))
    }

    fn release_or_debug(&self) -> &str {
        if self.cargo.release {
            "release"
        } else {
            "debug"
        }
    }

    pub fn output_bin(&self, target: &Target) -> Result<PathBuf, Error> {
        self.target_dir().map(|t| {
            t.join(format!(
                "{}_opt.wasm",
                Cmd::bin_name(target).trim_end_matches(".wasm")
            ))
        })
    }

    // pub fn bin_dir(&self) -> Result<PathBuf, Error> {
    //     Ok(self.cargo.target_dir().map_err(|_|Error::Cargo)?.join("res"))
    // }

    pub fn target_dir(&self) -> Result<PathBuf, Error> {
        Ok(self
            .cargo
            .target_dir()
            .map_err(|_| Error::Cargo("Filed to find target_dir".to_string()))?
            .join("wasm32-unknown-unknown")
            .join(self.release_or_debug()))
    }

    pub fn orig_bin(&self, target: &Target) -> Result<PathBuf, Error> {
        self.target_dir().map(|t| t.join(Cmd::bin_name(target)))
    }

    pub fn should_rebuild(&self, t: &Target) -> Result<bool, Error> {
        let orig_bin = &self.orig_bin(t)?;
        let output_bin = &self.output_bin(t)?;

        Ok(get_time(output_bin)? < get_time(orig_bin)?)
    }
}

fn get_time(path: &Path) -> Result<FileTime, Error> {
    fs::metadata(path)
        .as_ref()
        .map_err(|_| Error::Cargo(format!("failed to time for {}", path.to_string_lossy())))
        .map(FileTime::from_last_modification_time)
}

fn read_module(filename: &PathBuf) -> binaryen::Module {
    let mut f = File::open(filename).expect("file not found");
    let mut contents = Vec::new();
    f.read_to_end(&mut contents)
        .expect("something went wrong reading the file");

    binaryen::Module::read(&contents).expect("something went wrong parsing the file")
}

fn write_module(filename: &PathBuf, wasm: &[u8]) {
    let mut f = File::create(filename).expect("failed to create output");
    f.write_all(wasm).expect("failed to write file");
}

fn optimize(input: &PathBuf, output: &PathBuf) {
    let mut module = read_module(input);
    let codegen_config = binaryen::CodegenConfig {
        optimization_level: 2,
        shrink_level: 2,
        debug_info: true,
    };
    module.optimize(&codegen_config);

    let optimized_wasm = module.write();
    write_module(output, &optimized_wasm);
}
