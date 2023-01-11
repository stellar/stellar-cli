use clap::{ArgEnum, Parser};
use soroban_env_host::xdr::{self};

#[derive(Parser, Debug)]
pub struct Cmd {
    /// XDR type to decode to
    #[clap(long, possible_values(xdr::TypeVariant::VARIANTS_STR))]
    r#type: xdr::TypeVariant,
    /// XDR (base64 encoded) to decode
    #[clap(long)]
    xdr: String,
    /// Type of output
    #[clap(long, arg_enum, default_value_t)]
    output: Output,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, ArgEnum)]
pub enum Output {
    // Default structured output
    Default,
    /// Json representation
    Json,
}

impl Default for Output {
    fn default() -> Self {
        Self::Default
    }
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("parsing xdr: {0}")]
    Xdr(#[from] xdr::Error),
    #[error("generating json: {0}")]
    Json(#[from] serde_json::Error),
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        let value =
            xdr::Type::from_xdr_base64(self.r#type, self.xdr.clone()).map_err(Error::Xdr)?;

        match self.output {
            Output::Default => println!("{value:#?}"),
            Output::Json => println!(
                "{}",
                serde_json::to_string_pretty(&value).map_err(Error::Json)?
            ),
        }

        Ok(())
    }
}
