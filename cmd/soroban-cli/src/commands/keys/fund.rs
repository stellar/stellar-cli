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
    pub address: public_key::Args,
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let print = Print::new(global_args.quiet);
        let addr = self.address.public_key().await?;
        let network = self.network.get(&self.address.locator)?;
        let formatted_name = self.address.name.to_string();
        network.fund_address(&addr).await?;
        print.checkln(format!(
            "Account {} funded on {:?}",
            formatted_name, network.network_passphrase
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
    fn fund_does_not_accept_ledger_flag() {
        let err = Cmd::try_parse_from(["fund", PUBLIC_KEY, "--ledger"])
            .expect_err("`--ledger` belongs to `keys address` only");
        assert_eq!(err.kind(), clap::error::ErrorKind::UnknownArgument);
    }
}
