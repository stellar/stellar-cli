use clap::Parser;
use std::fmt::Debug;

use crate::{
    commands::global,
    config::{
        self,
        locator::{self, KeyType},
        network::{Network, DEFAULTS as DEFAULT_NETWORKS},
    },
    print::Print,
    rpc,
    upgrade_check::has_available_upgrade,
};

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub config_locator: locator::Args,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Locator(#[from] locator::Error),

    #[error(transparent)]
    Network(#[from] config::network::Error),

    #[error(transparent)]
    RpcClient(#[from] rpc::Error),
}

impl Cmd {
    pub async fn run(&self, _global_args: &global::Args) -> Result<(), Error> {
        let print = Print::new(false);

        check_version(&print).await?;
        inspect_networks(&print, &self.config_locator).await?;

        Ok(())
    }
}

async fn print_network(
    default: bool,
    print: &Print,
    name: &str,
    network: &Network,
) -> Result<(), Error> {
    let client = network.rpc_client()?;
    let version_info = client.get_version_info().await?;

    let prefix = if default {
        "Default network"
    } else {
        "Network"
    };

    print.globeln(format!(
        "{prefix} {name:?} ({}): protocol {}",
        network.rpc_url, version_info.protocol_version
    ));

    Ok(())
}

async fn inspect_networks(print: &Print, config_locator: &locator::Args) -> Result<(), Error> {
    let saved_networks = KeyType::Network.list_paths(&config_locator.local_and_global()?)?;
    let default_networks = DEFAULT_NETWORKS
        .into_iter()
        .map(|(name, network)| ((*name).to_string(), network.into()));

    for (name, network) in default_networks {
        // Skip default mainnet, because it has no default rpc url.
        if name == "mainnet" {
            continue;
        }

        if print_network(true, print, &name, &network).await.is_err() {
            print.warnln(format!(
                "Default network {name:?} ({}) is unreachable",
                network.rpc_url
            ));
            continue;
        }
    }

    for (name, _) in &saved_networks {
        if let Ok(network) = config_locator.read_network(name) {
            if print_network(false, print, name, &network).await.is_err() {
                print.warnln(format!(
                    "Network {name:?} ({}) is unreachable",
                    network.rpc_url
                ));
                continue;
            }
        }
    }

    Ok(())
}

async fn check_version(print: &Print) -> Result<(), Error> {
    if let Ok((upgrade_available, current_version, latest_version)) =
        has_available_upgrade(false).await
    {
        if upgrade_available {
            print.warnln(format!(
                "A new release of Stellar CLI is available: {current_version} -> {latest_version}"
            ));
        } else {
            print.checkln(format!(
                "You are using the latest version of Stellar CLI: {current_version}"
            ));
        }
    }

    Ok(())
}
