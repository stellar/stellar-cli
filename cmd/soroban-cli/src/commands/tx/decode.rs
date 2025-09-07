use clap::ValueEnum;
use std::ffi::OsString;
use stellar_xdr::{cli::Channel, curr::TypeVariant};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Cli(#[from] stellar_xdr::cli::decode::Error),
}

/// Decode a transaction envelope from XDR to JSON
#[derive(Debug, clap::Parser, Clone, Default)]
pub struct Cmd {
    /// XDR or files containing XDR to decode, or stdin if empty
    #[arg()]
    pub input: Vec<OsString>,
    // Input format
    #[arg(long = "input", value_enum, default_value_t)]
    pub input_format: InputFormat,
    // Output format
    #[arg(long = "output", value_enum, default_value_t)]
    pub output_format: OutputFormat,
}

#[derive(Default, Clone, Copy, Debug, Eq, Hash, PartialEq, ValueEnum)]
pub enum InputFormat {
    #[default]
    SingleBase64,
    Single,
}

impl From<InputFormat> for stellar_xdr::cli::decode::InputFormat {
    fn from(v: InputFormat) -> Self {
        match v {
            InputFormat::SingleBase64 => Self::SingleBase64,
            InputFormat::Single => Self::Single,
        }
    }
}

#[derive(Default, Clone, Copy, Debug, Eq, Hash, PartialEq, ValueEnum)]
pub enum OutputFormat {
    #[default]
    Json,
    JsonFormatted,
}

impl From<OutputFormat> for stellar_xdr::cli::decode::OutputFormat {
    fn from(v: OutputFormat) -> Self {
        match v {
            OutputFormat::Json => Self::Json,
            OutputFormat::JsonFormatted => Self::JsonFormatted,
        }
    }
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        let cmd = stellar_xdr::cli::decode::Cmd {
            input: self.input.clone(),
            r#type: TypeVariant::TransactionEnvelope.name().to_string(),
            input_format: self.input_format.into(),
            output_format: self.output_format.into(),
        };
        cmd.run(&Channel::Curr)?;
        Ok(())
    }
}
