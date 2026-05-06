use crate::{commands::global, config::network, print::Print};

use super::public_key;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Address(#[from] public_key::Error),
    #[error(transparent)]
    Network(#[from] network::Error),
}

#[derive(Debug, clap::Parser, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub network: network::Args,
    /// Address to fund
    #[command(flatten)]
    pub address: public_key::Cmd,
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let print = Print::new(global_args.quiet);
        let addr = self.address.public_key().await?;
        let network = self.network.get(&self.address.locator)?;
        let label = self
            .address
            .name
            .as_ref()
            .map_or_else(|| addr.to_string(), ToString::to_string);
        network.fund_address(&addr).await?;
        print.checkln(format!(
            "Account {} funded on {:?}",
            label, network.network_passphrase
        ));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    const PUBLIC_KEY: &str = "GAKSH6AD2IPJQELTHIOWDAPYX74YELUOWJLI2L4RIPIPZH6YQIFNUSDC";

    #[test]
    fn ledger_flag_parses_without_name() {
        let cmd = Cmd::try_parse_from(["fund", "--ledger"]).expect("--ledger alone parses");
        assert!(cmd.address.ledger);
        assert!(cmd.address.name.is_none());
    }

    #[test]
    fn ledger_flag_with_hd_path_parses() {
        let cmd = Cmd::try_parse_from(["fund", "--ledger", "--hd-path", "5"]).unwrap();
        assert!(cmd.address.ledger);
        assert_eq!(cmd.address.hd_path, Some(5));
    }

    #[test]
    fn ledger_flag_conflicts_with_name() {
        let err = Cmd::try_parse_from(["fund", PUBLIC_KEY, "--ledger"])
            .expect_err("--ledger + name must conflict");
        assert_eq!(err.kind(), clap::error::ErrorKind::ArgumentConflict);
    }

    #[test]
    fn missing_name_without_ledger_is_rejected() {
        let err = Cmd::try_parse_from(["fund"]).expect_err("name is required without --ledger");
        assert_eq!(err.kind(), clap::error::ErrorKind::MissingRequiredArgument);
    }
}
