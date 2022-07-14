use clap::Parser;
use std::{fmt::Debug, fs, io, io::Cursor, str::Utf8Error};
use stellar_contract_env_host::{
    xdr::{self, ReadXdr, ScSpecEntry, ScEnvMetaEntry},
    Host, HostError, Vm,
};

#[derive(Parser, Debug)]
pub struct Inspect {
    #[clap(long, parse(from_os_str))]
    file: std::path::PathBuf,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("xdr")]
    Xdr(#[from] xdr::Error),
    #[error("io")]
    Io(#[from] io::Error),
    #[error("host")]
    Host(#[from] HostError),
    #[error("utf8")]
    Utf8Error(#[from] Utf8Error),
}

impl Inspect {
    pub fn run(&self) -> Result<(), Error> {
        let contents = fs::read(&self.file)?;
        let h = Host::default();
        let vm = Vm::new(&h, [0; 32].into(), &contents)?;
        println!("File: {}", self.file.to_string_lossy());
        println!("Functions:");
        for f in vm.functions() {
            println!(
                " • {} ({}) -> ({})",
                f.name,
                vec!["val"; f.param_count].join(", "),
                vec!["res"; f.result_count].join(", ")
            );
        }
        if let Some(env_meta) = vm.custom_section("contractenvmetav0") {
            println!("Env Meta: {}", base64::encode(env_meta));
            let mut cursor = Cursor::new(env_meta);
            for env_meta_entry in ScEnvMetaEntry::read_xdr_iter(&mut cursor) {
                match env_meta_entry? {
                    ScEnvMetaEntry::ScEnvMetaKindInterfaceVersion(v) => println!(
                        " • Interface Version: {}",
                        v,
                    ),
                }
            }
        } else {
            println!("Contract Spec: None");
        }
        if let Some(spec) = vm.custom_section("contractspecv0") {
            println!("Contract Spec: {}", base64::encode(spec));
            let mut cursor = Cursor::new(spec);
            for spec_entry in ScSpecEntry::read_xdr_iter(&mut cursor) {
                match spec_entry? {
                    ScSpecEntry::FunctionV0(f) => println!(
                        " • Function: {} ({:?}) -> ({:?})",
                        f.name.to_string()?,
                        f.input_types.as_slice(),
                        f.output_types.as_slice(),
                    ),
                    ScSpecEntry::UdtUnionV0(udt) => {
                        println!(" • Union: {:?}", udt);
                    }
                    ScSpecEntry::UdtStructV0(udt) => {
                        println!(" • Struct: {:?}", udt);
                    }
                }
            }
        } else {
            println!("Contract Spec: None");
        }
        Ok(())
    }
}
