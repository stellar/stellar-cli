use clap::{arg, command, Parser};

use crate::commands::config;
use crate::commands::lab;
use crate::commands::lab::token::wrap::{get_contract_id, parse_asset};

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    /// ID of the Stellar classic asset to wrap, e.g. "USDC:G...5"
    #[arg(long)]
    pub asset: String,

    #[command(flatten)]
    pub config: config::Args,
}

impl Cmd {
    pub fn run(&self) -> Result<(), lab::token::wrap::Error> {
        let asset = parse_asset(&self.asset)?;
        let network = self.config.get_network()?;
        let contract_id = get_contract_id(&asset, &network.network_passphrase)?;
        let contract_key = stellar_strkey::Contract(contract_id.0).to_string();
        println!("Id: {contract_id}");
        println!("Key: {contract_key}");
        Ok(())
    }
}
