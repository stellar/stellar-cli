use std::str::FromStr;

use clap::{Parser, Subcommand};
use stellar_strkey::{
    cli::{decode, encode, version, zero, Error, RunOpts},
    ed25519, Decoded, Strkey, Unredacted,
};

// Wraps the embedded strkey CLI (`stellar_strkey::cli::Root`), which only reads
// the input for `decode` and `encode` from stdin, so that the input can also be
// passed as an argument, as it could before stellar-strkey v0.0.18, avoiding a
// breaking change.
//
// TODO: Remove at the next major version (v29/30), and embed
// `stellar_strkey::cli::Root` directly again.
#[derive(Parser, Debug, Clone)]
#[command(infer_subcommands = true)]
pub struct Cmd {
    #[command(subcommand)]
    command: SubCommand,
    /// Suppress stderr log and warning output
    #[arg(long, short = 'q', global = true)]
    quiet: bool,
}

#[derive(Subcommand, Debug, Clone)]
enum SubCommand {
    /// Decode strkey
    Decode {
        /// Strkey to decode, or stdin if empty
        strkey: Option<String>,
    },
    /// Encode strkey
    Encode {
        /// JSON for Strkey to encode, or stdin if empty
        json: Option<String>,
    },
    /// Generate the zero strkey
    Zero(zero::Cmd),
    /// Print version information
    Version,
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        let opts = RunOpts { quiet: self.quiet };
        match &self.command {
            SubCommand::Decode { strkey: Some(s) } => decode_arg(s, &opts)?,
            SubCommand::Decode { strkey: None } => decode::Cmd {}.run(&opts)?,
            SubCommand::Encode { json: Some(j) } => encode_arg(j, &opts)?,
            SubCommand::Encode { json: None } => encode::Cmd {}.run(&opts)?,
            SubCommand::Zero(cmd) => cmd.run(),
            SubCommand::Version => version::Cmd::run(),
        }
        Ok(())
    }
}

// Same as `decode::Cmd::run`, but for input passed as an argument.
fn decode_arg(input: &str, opts: &RunOpts) -> Result<(), decode::Error> {
    let input = input.trim();
    // `S…` strkeys are decoded via `ed25519::PrivateKey` directly; the
    // Strkey enum intentionally excludes that variant.
    let json = if let Ok(k) = Strkey::from_str(input) {
        serde_json::to_string_pretty(&Decoded(&k)).unwrap()
    } else {
        let pk = ed25519::PrivateKey::from_str(input)
            .map_err(|e| decode::Error::Decode(input.to_string(), e))?;
        if !opts.quiet {
            warn_private_key();
        }
        serde_json::to_string_pretty(&serde_json::json!({
            "private_key_ed25519": Decoded(Unredacted(&pk)),
        }))
        .unwrap()
    };
    println!("{json}");
    Ok(())
}

// Same as `encode::Cmd::run`, but for input passed as an argument.
fn encode_arg(input: &str, opts: &RunOpts) -> Result<(), encode::Error> {
    // Peek at the variant key: `private_key_ed25519` is handled outside
    // the Strkey enum and routed through `ed25519::PrivateKey`.
    let value: serde_json::Value = serde_json::from_str(input).map_err(encode::Error::Json)?;
    let pk_value = value
        .as_object()
        .filter(|m| m.len() == 1)
        .and_then(|m| m.get("private_key_ed25519"))
        .cloned();
    if let Some(pk_value) = pk_value {
        let Decoded(Unredacted(pk)): Decoded<Unredacted<ed25519::PrivateKey>> =
            serde_json::from_value(pk_value).map_err(encode::Error::Json)?;
        if !opts.quiet {
            warn_private_key();
        }
        println!("{}", Unredacted(&pk));
    } else {
        let Decoded(strkey): Decoded<Strkey> =
            serde_json::from_value(value).map_err(encode::Error::Json)?;
        println!("{strkey}");
    }
    Ok(())
}

// Same warning as the embedded CLI prints, which it doesn't expose.
fn warn_private_key() {
    eprintln!("⚠️  Warning: output contains a private key with secret material. Handle with care.");
}
