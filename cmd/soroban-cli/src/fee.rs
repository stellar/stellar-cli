use crate::{commands::HEADING_RPC, rpc};
use clap::arg;
use soroban_env_host::{
    fees::FeeConfiguration,
    xdr::{self, ReadXdr},
};

#[derive(Debug, clap::Args, Clone)]
#[group(skip)]
pub struct Args {
    /// fee amount for transaction, in stroops. 1 stroop = 0.0000001 xlm
    #[arg(long, default_value = "100", env = "SOROBAN_FEE", help_heading = HEADING_RPC)]
    pub fee: u32,
}

impl Default for Args {
    fn default() -> Self {
        Self { fee: 100 }
    }
}

pub async fn get_fee_configuration(
    client: &rpc::Client,
) -> Result<(FeeConfiguration, u32), rpc::Error> {
    let response = client
        .get_ledger_entries(
            &vec![
                xdr::ConfigSettingId::ContractComputeV0,
                xdr::ConfigSettingId::ContractLedgerCostV0,
                xdr::ConfigSettingId::ContractHistoricalDataV0,
                xdr::ConfigSettingId::ContractMetaDataV0,
                xdr::ConfigSettingId::ContractBandwidthV0,
            ]
            .iter()
            .map(|config_setting_id| {
                xdr::LedgerKey::ConfigSetting(xdr::LedgerKeyConfigSetting {
                    config_setting_id: *config_setting_id,
                })
            })
            .collect::<Vec<_>>(),
        )
        .await?;

    let entries = response
        .entries
        .unwrap_or_default()
        .iter()
        .map(|e| xdr::LedgerEntryData::from_xdr_base64(&e.xdr))
        .map(|e| match e {
            Ok(xdr::LedgerEntryData::ConfigSetting(config_setting)) => Ok(config_setting),
            Err(e) => Err(rpc::Error::Xdr(e)),
            Ok(_) => Err(rpc::Error::Xdr(xdr::Error::Invalid)),
        })
        .collect::<Result<Vec<_>, _>>()?;

    if entries.len() != 5 {
        return Err(rpc::Error::InvalidResponse);
    }

    let [
        xdr::ConfigSettingEntry::ContractComputeV0(compute),
        xdr::ConfigSettingEntry::ContractLedgerCostV0(ledger_cost),
        xdr::ConfigSettingEntry::ContractHistoricalDataV0(historical_data),
        xdr::ConfigSettingEntry::ContractMetaDataV0(metadata),
        xdr::ConfigSettingEntry::ContractBandwidthV0(bandwidth),
    ] =     &entries[..] else {
        return Err(rpc::Error::InvalidResponse);
    };

    // Taken from Stellar Core's InitialSorobanNetworkConfig in NetworkConfig.h
    let fee_configuration = FeeConfiguration {
        fee_per_instruction_increment: compute.fee_rate_per_instructions_increment,
        fee_per_read_entry: ledger_cost.fee_read_ledger_entry,
        fee_per_write_entry: ledger_cost.fee_write_ledger_entry,
        fee_per_read_1kb: ledger_cost.fee_read1_kb,
        fee_per_write_1kb: ledger_cost.fee_write1_kb,
        fee_per_historical_1kb: historical_data.fee_historical1_kb,
        fee_per_metadata_1kb: metadata.fee_extended_meta_data1_kb,
        fee_per_propagate_1kb: bandwidth.fee_propagate_data1_kb,
    };

    let latest_ledger_seq = response
        .latest_ledger
        .parse::<u32>()
        .map_err(|_| rpc::Error::Xdr(xdr::Error::Invalid))?;
    Ok((fee_configuration, latest_ledger_seq))
}
