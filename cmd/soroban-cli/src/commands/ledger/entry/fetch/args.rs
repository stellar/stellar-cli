use crate::{
    config::{
        locator,
        network::{self, Network},
    },
    rpc,
    xdr::{LedgerKey, MuxedAccount},
};

#[derive(Debug, clap::Args, Clone)]
#[group(skip)]
pub struct Args {
    #[command(flatten)]
    pub network: network::Args,

    #[command(flatten)]
    pub locator: locator::Args,

    /// Format of the output
    #[arg(long, default_value = "json")]
    pub output: OutputFormat,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Network(#[from] network::Error),
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
    #[error(transparent)]
    Rpc(#[from] rpc::Error),
}

/// Resolve an `--account` alias or address to a muxed account. A secret key or
/// seed phrase mistyped where an account was expected is concealed so it never
/// reaches the terminal, logs, or JSON output.
pub fn resolve_account(
    locator: &locator::Args,
    account: &str,
    hd_path: Option<u32>,
) -> Result<MuxedAccount, locator::Error> {
    Ok(locator
        .read_key(account)
        .map_err(locator::Error::conceal_secret_input)?
        .muxed_account(hd_path)?)
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, clap::ValueEnum, Default)]
pub enum OutputFormat {
    /// JSON output of the ledger entry with parsed XDRs (one line, not formatted)
    #[default]
    Json,
    /// Formatted (multiline) JSON output of the ledger entry with parsed XDRs
    JsonFormatted,
    /// Original RPC output (containing XDRs)
    Xdr,
}

impl Args {
    pub fn network(&self) -> Result<Network, Error> {
        Ok(self.network.get(&self.locator)?)
    }

    pub async fn run(&self, ledger_keys: Vec<LedgerKey>) -> Result<(), Error> {
        let network = self.network.get(&self.locator)?;
        let client = network.rpc_client()?;
        match self.output {
            OutputFormat::Json => {
                let resp = client.get_full_ledger_entries(&ledger_keys).await?;
                println!("{}", serde_json::to_string(&resp)?);
            }
            OutputFormat::Xdr => {
                let resp = client.get_ledger_entries(&ledger_keys).await?;
                println!("{}", serde_json::to_string(&resp)?);
            }
            OutputFormat::JsonFormatted => {
                let resp = client.get_full_ledger_entries(&ledger_keys).await?;
                println!("{}", serde_json::to_string_pretty(&resp)?);
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::secret::Secret;

    const TEST_SECRET_KEY: &str = "SBF5HLRREHMS36XZNTUSKZ6FTXDZGNXOHF4EXKUL5UCWZLPBX3NGJ4BH";
    // TEST_SECRET_KEY with its last character changed: a bad checksum that fails to
    // parse as a key yet clears name validation and reaches the config lookup.
    const MISTYPED_SECRET: &str = "SBF5HLRREHMS36XZNTUSKZ6FTXDZGNXOHF4EXKUL5UCWZLPBX3NGJ4BX";
    // A seed phrase with an invalid final word: fails to parse and fails name
    // validation on its spaces.
    const MALFORMED_SEED: &str =
        "illness spike retreat truth genius clock brain pass fit cave bargain xyzzy";

    fn locator_with_identity() -> (tempfile::TempDir, locator::Args) {
        let dir = tempfile::tempdir().unwrap();
        let locator = locator::Args {
            config_dir: Some(dir.path().to_path_buf()),
        };
        let secret = Secret::SecretKey {
            secret_key: TEST_SECRET_KEY.to_string(),
        };
        locator.write_identity("alice", &secret).unwrap();
        (dir, locator)
    }

    #[test]
    fn resolve_account_does_not_leak_secret_bearing_input() {
        let (_dir, locator) = locator_with_identity();
        for input in [MISTYPED_SECRET, MALFORMED_SEED] {
            let err = resolve_account(&locator, input, None).unwrap_err();
            assert!(
                !err.to_string().contains(input),
                "error leaked secret-bearing input: {err}"
            );
        }
    }
}
