use clap::Parser;
use std::{fmt::Debug, fs, io, io::Cursor, str::Utf8Error};
use stellar_contract_env_host::{Host, HostError, Vm};
use stellar_xdr::{ReadXdr, SpecEntry, SpecEntryFunction};

#[derive(Parser, Debug)]
pub struct Inspect {
    #[clap(long, parse(from_os_str))]
    file: std::path::PathBuf,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("xdr")]
    Xdr(#[from] stellar_xdr::Error),
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
        if let Some(spec) = vm.custom_section("contractspecv0") {
            println!("Contract Spec: {}", base64::encode(spec));
            let mut cursor = Cursor::new(spec);
            while let Ok(spec_entry) = SpecEntry::read_xdr(&mut cursor) {
                match spec_entry {
                    SpecEntry::Function(SpecEntryFunction::V0(f)) => println!(
                        " • Function: {} ({:?}) -> ({:?})",
                        std::str::from_utf8(f.name.as_slice())?,
                        f.input_types.as_slice(),
                        f.output_types.as_slice(),
                    ),
                    SpecEntry::Type(_) => todo!(),
                }
            }
        } else {
            println!("Contract Spec: None");
        }
        Ok(())
    }
}
