use stellar_xdr::cli::{decode::OutputFormat, Channel};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Cli(#[from] stellar_xdr::cli::decode::Error),
}

/// Decode a transaction envelope to JSON
#[derive(Debug, clap::Parser, Clone, Default)]
#[group(skip)]
pub struct Cmd {
    // Output format
    #[arg(long, value_enum, default_value_t)]
    pub output: OutputFormat,
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        let cmd = stellar_xdr::cli::decode::Cmd {
            files: vec![],
            r#type: "TransactionEnvelope".to_string(),
            input: stellar_xdr::cli::decode::InputFormat::SingleBase64,
            output: self.output,
        };
        cmd.run(&Channel::Curr)?;
        Ok(())
    }
}
