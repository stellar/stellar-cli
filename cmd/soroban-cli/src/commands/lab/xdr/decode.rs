use clap::{
    arg,
    builder::{PossibleValuesParser, TypedValueParser},
    Parser, ValueEnum,
};
use core::str::FromStr;
use soroban_env_host::xdr;

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    /// XDR type to decode to
    #[arg(
        long,
        value_parser =
            PossibleValuesParser::new(xdr::TypeVariant::VARIANTS_STR)
                .try_map(|s| xdr::TypeVariant::from_str(&s))
    )]
    r#type: xdr::TypeVariant,
    /// XDR (base64 encoded) to decode
    #[arg(long)]
    xdr: String,
    /// Type of output
    #[arg(long, value_enum, default_value_t)]
    output: Output,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, ValueEnum)]
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
