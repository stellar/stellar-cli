use clap::Parser;
use rustc_version::version;
use semver::Version;
use std::fmt::Debug;
use std::process::Command;

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

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl Cmd {
    pub async fn run(&self, _global_args: &global::Args) -> Result<(), Error> {
        let print = Print::new(false);

        check_version(&print).await?;
        check_rust_version(&print);
        check_wasm_target(&print);
        show_config_path(&print, &self.config_locator)?;
        show_xdr_version(&print);
        inspect_networks(&print, &self.config_locator).await?;

        Ok(())
    }
}

fn show_config_path(print: &Print, config_locator: &locator::Args) -> Result<(), Error> {
    let global_path = config_locator.global_config_path()?;

    print.gearln(format!(
        "Config directory: {}",
        global_path.to_string_lossy()
    ));

    Ok(())
}

fn show_xdr_version(print: &Print) {
    let xdr = stellar_xdr::VERSION;

    print.infoln(format!("XDR version: {}", xdr.xdr_curr));
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

    print.globeln(format!("{prefix} {name:?} ({})", network.rpc_url,));
    print.blankln(format!(" protocol {}", version_info.protocol_version));
    print.blankln(format!(" rpc {}", version_info.version));

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
        }
    }

    for (name, _) in &saved_networks {
        if let Ok(network) = config_locator.read_network(name) {
            if print_network(false, print, name, &network).await.is_err() {
                print.warnln(format!(
                    "Network {name:?} ({}) is unreachable",
                    network.rpc_url
                ));
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

fn check_rust_version(print: &Print) {
    match version() {
        Ok(rust_version) => {
            let v184 = Version::parse("1.84.0").unwrap();
            let v182 = Version::parse("1.82.0").unwrap();

            if rust_version >= v182 && rust_version < v184 {
                print.errorln(format!(
                    "Rust {rust_version} cannot be used to build contracts"
                ));
            } else {
                print.infoln(format!("Rust version: {rust_version}"));
            }
        }
        Err(_) => {
            print.warnln("Could not determine Rust version".to_string());
        }
    }
}

fn check_wasm_target(print: &Print) {
    let expected_target = get_expected_wasm_target();

    let Ok(output) = Command::new("rustup")
        .args(["target", "list", "--installed"])
        .output()
    else {
        print.warnln("Could not retrieve Rust targets".to_string());
        return;
    };

    if output.status.success() {
        let targets = String::from_utf8_lossy(&output.stdout);

        if targets.lines().any(|line| line.trim() == expected_target) {
            print.checkln(format!("Rust target `{expected_target}` is installed"));
        } else {
            print.errorln(format!("Rust target `{expected_target}` is not installed"));
        }
    } else {
        print.warnln("Could not retrieve Rust targets".to_string());
    }
}

fn get_expected_wasm_target() -> String {
    let Ok(current_version) = version() else {
        return "wasm32v1-none".into();
    };

    let v184 = Version::parse("1.84.0").unwrap();

    if current_version < v184 {
        "wasm32-unknown-unknown".into()
    } else {
        "wasm32v1-none".into()
    }
}
