use crate::{
    commands::config::{address, locator},
    config::UnresolvedMuxedAccount,
    signer::ledger,
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Address(#[from] address::Error),

    #[error(transparent)]
    Ledger(#[from] ledger::Error),
}

#[derive(Debug, clap::Parser, Clone)]
#[group(skip)]
pub struct Cmd {
    /// Name of identity to lookup. Required unless `--ledger` is provided.
    #[arg(required_unless_present = "ledger")]
    pub name: Option<UnresolvedMuxedAccount>,

    /// If identity is a seed phrase use this hd path, default is 0.
    /// With --ledger this is the Ledger account index (default 0).
    #[arg(long)]
    pub hd_path: Option<u32>,

    /// Derive the address from a connected Ledger hardware wallet at
    /// `m/44'/148'/N'`, where `N` defaults to 0 and can be set with
    /// `--hd-path`.
    #[arg(long, conflicts_with = "name")]
    pub ledger: bool,

    #[command(flatten)]
    pub locator: locator::Args,
}

impl Cmd {
    pub async fn run(&self) -> Result<(), Error> {
        println!("{}", self.public_key().await?);
        Ok(())
    }

    pub async fn public_key(&self) -> Result<stellar_strkey::ed25519::PublicKey, Error> {
        if self.ledger {
            return Ok(ledger::new(self.hd_path.unwrap_or_default())
                .await?
                .public_key()
                .await?);
        }
        let name = self
            .name
            .as_ref()
            .expect("clap requires `name` unless --ledger is set");
        Ok(public_key_from_muxed(
            name.resolve_muxed_account(&self.locator, self.hd_path)?,
        ))
    }
}

fn public_key_from_muxed(
    muxed: soroban_sdk::xdr::MuxedAccount,
) -> stellar_strkey::ed25519::PublicKey {
    let bytes = match muxed {
        soroban_sdk::xdr::MuxedAccount::Ed25519(uint256) => uint256.0,
        soroban_sdk::xdr::MuxedAccount::MuxedEd25519(muxed_account) => muxed_account.ed25519.0,
    };
    stellar_strkey::ed25519::PublicKey(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    const PUBLIC_KEY: &str = "GAKSH6AD2IPJQELTHIOWDAPYX74YELUOWJLI2L4RIPIPZH6YQIFNUSDC";

    #[test]
    fn ledger_flag_parses_without_name() {
        let cmd = Cmd::try_parse_from(["address", "--ledger"]).expect("--ledger alone parses");
        assert!(cmd.ledger);
        assert!(cmd.name.is_none());
        assert_eq!(cmd.hd_path, None);
    }

    #[test]
    fn ledger_flag_with_hd_path_parses() {
        let cmd = Cmd::try_parse_from(["address", "--ledger", "--hd-path", "5"]).unwrap();
        assert!(cmd.ledger);
        assert_eq!(cmd.hd_path, Some(5));
    }

    #[test]
    fn ledger_flag_conflicts_with_name() {
        let err = Cmd::try_parse_from(["address", PUBLIC_KEY, "--ledger"])
            .expect_err("--ledger + name must conflict");
        assert_eq!(err.kind(), clap::error::ErrorKind::ArgumentConflict);
    }

    #[test]
    fn missing_name_without_ledger_is_rejected() {
        let err = Cmd::try_parse_from(["address"]).expect_err("name is required without --ledger");
        assert_eq!(err.kind(), clap::error::ErrorKind::MissingRequiredArgument);
    }

    #[test]
    fn name_without_ledger_parses() {
        let cmd = Cmd::try_parse_from(["address", PUBLIC_KEY]).unwrap();
        assert!(!cmd.ledger);
        assert!(cmd.name.is_some());
    }
}
