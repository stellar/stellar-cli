use super::locator;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Config(#[from] locator::Error),
}

#[derive(Debug, clap::Parser, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub config_locator: locator::Args,
    /// Get more info about the networks
    #[arg(long, short = 'l')]
    pub long: bool,
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        let sep = if self.long { "\n\n" } else { "\n" };
        let res = if self.long { self.ls_l() } else { self.ls() }?.join(sep);
        println!("{res}");
        Ok(())
    }

    pub fn ls(&self) -> Result<Vec<String>, Error> {
        Ok(self.config_locator.list_networks()?)
    }

    pub fn ls_l(&self) -> Result<Vec<String>, Error> {
        Ok(self
            .config_locator
            .list_networks_long()?
            .iter()
            .filter_map(|(name, network, _)| {
                let headers = if network.rpc_headers.is_empty() {
                    " not set".to_string()
                } else {
                    let lines: Vec<String> = network
                        .rpc_headers
                        .iter()
                        .map(|(k, _)| format!("  {k}: <concealed>"))
                        .collect();
                    format!("\n{}", lines.join("\n"))
                };

                Some(format!(
                    "Name: {name}\nRPC url: {rpc_url}\nRPC headers:{headers}\nNetwork passphrase: {passphrase}",
                    rpc_url = network.rpc_url,
                    passphrase = network.network_passphrase,
                ))
            })
            .collect())
    }
}
