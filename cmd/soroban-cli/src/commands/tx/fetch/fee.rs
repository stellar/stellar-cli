use super::args;
use crate::{
    commands::global,
    config::network,
    rpc,
    xdr::{
        self, FeeBumpTransactionInnerTx, Hash, SorobanTransactionMetaExt, TransactionEnvelope,
        TransactionMeta, TransactionResult,
    },
};
use clap::{command, Parser};
use prettytable::{
    format::{FormatBuilder, LinePosition, LineSeparator, TableFormat},
    Cell, Row, Table,
};
use serde::{Deserialize, Serialize};
use soroban_rpc::GetTransactionResponse;

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    args: args::Args,

    /// Output format for fee command
    #[arg(long, default_value = "table")]
    pub output: FeeOutputFormat,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, clap::ValueEnum, Default)]
pub enum FeeOutputFormat {
    /// JSON output of the ledger entry with parsed XDRs (one line, not formatted)
    Json,
    /// Formatted (multiline) JSON output of the ledger entry with parsed XDRs
    JsonFormatted,
    /// Formatted in a table comparing fee types
    #[default]
    Table,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Network(#[from] network::Error),
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
    #[error(transparent)]
    Xdr(#[from] xdr::Error),
    #[error(transparent)]
    Args(#[from] args::Error),
    #[error("{message}")]
    NotSupported { message: String },
    #[error("transaction {tx_hash} not found on {network} network")]
    NotFound { tx_hash: Hash, network: String },
    #[error(transparent)]
    Rpc(#[from] rpc::Error),
    #[error("{field} is None, expected it to be Some")]
    None { field: String },
}

const DEFAULT_RESOURCE_FEE: i64 = 0; // this is the resource fee for non-soroban transactions
const FEE_CHARGED_TITLE: &str = "Fee Charged";
const RESOURCE_FEE_TITLE: &str = "Resource Fee";
const INCLUSION_FEE_TITLE: &str = "Inclusion Fee";
const NON_REFUNDABLE_TITLE: &str = "Non-Refundable";
const REFUNDABLE_TITLE: &str = "Refundable";
const FEE_PROPOSED_TITLE: &str = "Fee Proposed";
const REFUNDED_TITLE: &str = "Refunded";
const NON_REFUNDABLE_COMPONENTS: &str = "\n\ncpu instructions\nstorage read/write\ntx size";
const REFUNDABLE_COMPONENTS: &str = "\n\nreturn value\nstorage rent\nevents";

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let resp = self.args.fetch_transaction(global_args).await?;
        let fee_table = FeeTable::new_from_transaction_response(&resp)?;
        match self.output {
            FeeOutputFormat::Json => {
                println!("{}", serde_json::to_string(&fee_table)?);
            }
            FeeOutputFormat::JsonFormatted => {
                println!("{}", serde_json::to_string_pretty(&fee_table)?);
            }
            FeeOutputFormat::Table => {
                fee_table.print();
            }
        }

        Ok(())
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FeeTable {
    // proposed
    pub max_fee: i64,
    pub max_resource_fee: i64,
    pub max_inclusion_fee: i64,

    // actual
    pub fee_charged: i64,
    pub resource_fee_charged: i64,
    pub inclusion_fee_charged: i64,
    pub non_refundable_resource_fee_charged: i64,
    pub refundable_resource_fee_charged: i64,
}

impl FeeTable {
    fn new_from_transaction_response(resp: &GetTransactionResponse) -> Result<Self, Error> {
        let (tx_result, tx_meta, tx_envelope) = Self::destructure_tx_response(resp)?;

        // PROPOSED fEES
        let (max_fee, max_resource_fee, max_inclusion_fee) = Self::max_fees(&tx_envelope);

        // ACTUAL FEES
        let fee_charged = tx_result.fee_charged;

        // fee refunded
        // inclusion fee charged
        // resource fee charged
        // resource fee charged - non-refundable
        // resource fee charged - refundable
        let (non_refundable_resource_fee_charged, refundable_resource_fee_charged) =
            Self::resource_fees_charged(&tx_meta);
        let resource_fee_charged =
            non_refundable_resource_fee_charged + refundable_resource_fee_charged;
        let inclusion_fee_charged = fee_charged - resource_fee_charged;

        Ok(FeeTable {
            max_fee,
            max_resource_fee,
            max_inclusion_fee,
            fee_charged,
            resource_fee_charged,
            inclusion_fee_charged,
            non_refundable_resource_fee_charged,
            refundable_resource_fee_charged,
        })
    }

    fn destructure_tx_response(
        resp: &GetTransactionResponse,
    ) -> Result<(TransactionResult, TransactionMeta, TransactionEnvelope), Error> {
        let tx_result = resp.result.clone().ok_or(Error::None {
            field: "tx_result".to_string(),
        })?;
        let tx_meta = resp.result_meta.clone().ok_or(Error::None {
            field: "tx_meta".to_string(),
        })?;
        let tx_envelope = resp.envelope.clone().ok_or(Error::None {
            field: "tx_envelope".to_string(),
        })?;

        Ok((tx_result, tx_meta, tx_envelope))
    }

    // returns max fee proposed, max resource fee and max inclusion fee
    fn max_fees(tx_envelope: &TransactionEnvelope) -> (i64, i64, i64) {
        match tx_envelope {
            TransactionEnvelope::TxV0(transaction_v0_envelope) => {
                let fee: i64 = transaction_v0_envelope.tx.fee.into();
                (fee, DEFAULT_RESOURCE_FEE, fee - DEFAULT_RESOURCE_FEE)
            }
            TransactionEnvelope::Tx(transaction_v1_envelope) => {
                let tx = transaction_v1_envelope.tx.clone();
                let fee: i64 = tx.fee.into();
                let resource_fee = match tx.ext {
                    xdr::TransactionExt::V0 => DEFAULT_RESOURCE_FEE,
                    xdr::TransactionExt::V1(soroban_transaction_data) => {
                        soroban_transaction_data.resource_fee
                    }
                };

                (fee, resource_fee, fee - resource_fee)
            }
            TransactionEnvelope::TxFeeBump(fee_bump_transaction_envelope) => {
                let inner_tx_resource_fee: i64;
                let inner_tx_inclusion_fee: i64;
                match &fee_bump_transaction_envelope.tx.inner_tx {
                    FeeBumpTransactionInnerTx::Tx(tx_v1_envelope) => {
                        let inner_tx_fee = tx_v1_envelope.tx.fee;
                        inner_tx_resource_fee = match &tx_v1_envelope.tx.ext {
                            xdr::TransactionExt::V0 => DEFAULT_RESOURCE_FEE,
                            xdr::TransactionExt::V1(soroban_transaction_data) => {
                                soroban_transaction_data.resource_fee
                            }
                        };

                        inner_tx_inclusion_fee = inner_tx_fee as i64 - inner_tx_resource_fee;
                    }
                }
                // the is the top level fee bump tx fee
                let fee = fee_bump_transaction_envelope.tx.fee;

                // to calculate the total max inclusion fee for the fee bump tx, we are subtracting (inner_tx_resource_fee + inner_tx_inclusion_fee) from the max fee bump tx's max fee because the inner inclusion fee and resource fee will not be charged to the fee bump txn's fee source
                let total_estimated_inclusion_fee =
                    fee - (inner_tx_resource_fee + inner_tx_inclusion_fee);

                (fee, inner_tx_resource_fee, total_estimated_inclusion_fee)
            }
        }
    }

    fn resource_fees_charged(tx_meta: &TransactionMeta) -> (i64, i64) {
        let (non_refundable_resource_fee_charged, refundable_resource_fee_charged) =
            match tx_meta.clone() {
                TransactionMeta::V0(_) | TransactionMeta::V1(_) | TransactionMeta::V2(_) => {
                    (DEFAULT_RESOURCE_FEE, DEFAULT_RESOURCE_FEE)
                }
                TransactionMeta::V3(meta) => {
                    // ah, is this a classic feebump vs soroban fee bump issue?
                    if let Some(soroban_meta) = meta.soroban_meta {
                        match soroban_meta.ext {
                            SorobanTransactionMetaExt::V0 => {
                                (DEFAULT_RESOURCE_FEE, DEFAULT_RESOURCE_FEE)
                            }
                            SorobanTransactionMetaExt::V1(v1) => (
                                v1.total_non_refundable_resource_fee_charged,
                                v1.total_refundable_resource_fee_charged,
                            ),
                        }
                    } else {
                        (DEFAULT_RESOURCE_FEE, DEFAULT_RESOURCE_FEE)
                    }
                }
                TransactionMeta::V4(meta) => {
                    if let Some(soroban_meta) = meta.soroban_meta {
                        match soroban_meta.ext {
                            SorobanTransactionMetaExt::V0 => {
                                (DEFAULT_RESOURCE_FEE, DEFAULT_RESOURCE_FEE)
                            }
                            SorobanTransactionMetaExt::V1(v1) => (
                                v1.total_non_refundable_resource_fee_charged,
                                v1.total_refundable_resource_fee_charged,
                            ),
                        }
                    } else {
                        (DEFAULT_RESOURCE_FEE, DEFAULT_RESOURCE_FEE)
                    }
                }
            };

        (
            non_refundable_resource_fee_charged,
            refundable_resource_fee_charged,
        )
    }

    fn should_include_resource_fees(&self) -> bool {
      self.resource_fee_charged != 0 || self.max_resource_fee != 0
    }

    fn refundable_fee_proposed(&self) -> i64 {
        self.max_resource_fee - self.non_refundable_resource_fee_charged
    }

    fn refunded(&self) -> i64 {
        self.max_fee - self.fee_charged
    }

    fn table(&self) -> Table {
        let mut table = Table::new();
        table.set_format(Self::table_format());

        // Proposed
        table.add_row(Row::new(vec![Cell::new(&format!(
            "{FEE_PROPOSED_TITLE}: {}",
            self.max_fee
        ))
        .with_hspan(4)]));

        table.add_row(Row::new(vec![
            Cell::new(&format!(
                "{}: {}",
                INCLUSION_FEE_TITLE, self.max_inclusion_fee
            )),
            Cell::new(&format!("{RESOURCE_FEE_TITLE}: {}", self.max_resource_fee)).with_hspan(3),
        ]));

        table.add_row(Row::new(vec![
            Cell::new(&format!(
                "{}: {}",
                INCLUSION_FEE_TITLE, self.max_inclusion_fee
            )),
            Cell::new(&format!(
                "{NON_REFUNDABLE_TITLE}: {}{}",
                self.non_refundable_resource_fee_charged, NON_REFUNDABLE_COMPONENTS
            )),
            Cell::new(&format!(
                "{REFUNDABLE_TITLE}: {}{}",
                self.refundable_fee_proposed(),
                REFUNDABLE_COMPONENTS
            ))
            .with_hspan(2),
        ]));

        // Info
        table.add_row(Row::new(vec![Cell::new("👆 Proposed Fee  👇 Final Fee")
            .style_spec("c")
            .with_hspan(4)]));

        // Fees Charged
        if self.should_include_resource_fees() {
            table.add_row(Row::new(vec![
                Cell::new(&format!(
                    "{INCLUSION_FEE_TITLE}: {}",
                    self.inclusion_fee_charged
                )),
                Cell::new(&format!(
                    "{NON_REFUNDABLE_TITLE}: {}",
                    self.non_refundable_resource_fee_charged
                )),
                Cell::new(&format!(
                    "{REFUNDABLE_TITLE}: {}",
                    self.refundable_resource_fee_charged
                )),
                Cell::new(&format!("{REFUNDED_TITLE}: {}", self.refunded())),
            ]));

            table.add_row(Row::new(vec![
                Cell::new(&format!(
                    "{INCLUSION_FEE_TITLE}: {}",
                    self.inclusion_fee_charged
                )),
                Cell::new(&format!(
                    "{}: {}",
                    RESOURCE_FEE_TITLE, self.resource_fee_charged
                ))
                .with_hspan(2),
                Cell::new(&format!("{REFUNDED_TITLE}: {}", self.refunded())),
            ]));
        }

        table.add_row(Row::new(vec![
            Cell::new(&format!("{FEE_CHARGED_TITLE}: {}", self.fee_charged)).with_hspan(3),
            Cell::new(&format!("{REFUNDED_TITLE}: {}", self.refunded())),
        ]));

        table
    }

    fn print(&self) {
        self.table().printstd();
    }

    fn table_format() -> TableFormat {
        FormatBuilder::new()
            .column_separator('│')
            .borders('│')
            .separators(&[LinePosition::Top], LineSeparator::new('─', '─', '┌', '┐'))
            .separators(
                &[LinePosition::Intern],
                LineSeparator::new('─', '─', '├', '┤'),
            )
            .separators(
                &[LinePosition::Bottom],
                LineSeparator::new('─', '─', '└', '┘'),
            )
            .padding(1, 1)
            .build()
    }
}

#[cfg(test)]
mod test {
    use soroban_rpc::GetTransactionResponse;

    use super::*;

    #[test]
    fn soroban_tx_fee_table() {
        let resp = soroban_tx_response().unwrap();
        let fee_table = FeeTable::new_from_transaction_response(&resp).unwrap();

        // Proposed Fees
        let expected_max_fee = 105_447;
        let expected_max_resource_fee = 105_347;
        assert_eq!(fee_table.max_fee, expected_max_fee);
        assert_eq!(fee_table.max_resource_fee, expected_max_resource_fee);

        // Charged Fees
        let expected_fee_charged = 60_537;
        let expected_non_refundable_charged = 60_358;
        let expected_refundable_charged = 79;
        let expected_resource_fee_charged =
            expected_non_refundable_charged + expected_refundable_charged;
        let expected_inclusion_fee_charged = expected_fee_charged - expected_resource_fee_charged;

        assert!(fee_table.should_include_resource_fees());
        assert_eq!(fee_table.fee_charged, expected_fee_charged);
        assert_eq!(
            fee_table.resource_fee_charged,
            expected_resource_fee_charged
        );
        assert_eq!(
            fee_table.non_refundable_resource_fee_charged,
            expected_non_refundable_charged
        );
        assert_eq!(
            fee_table.refundable_resource_fee_charged,
            expected_refundable_charged
        );
        assert_eq!(
            fee_table.inclusion_fee_charged,
            expected_inclusion_fee_charged
        );
    }

    #[test]
    fn classic_tx_fee_table() {
        let resp = classic_tx_response().unwrap();
        let fee_table = FeeTable::new_from_transaction_response(&resp).unwrap();

        // Proposed Fees
        let expected_max_fee = 100;
        let expected_max_resource_fee = DEFAULT_RESOURCE_FEE;
        assert_eq!(fee_table.max_fee, expected_max_fee);
        assert_eq!(fee_table.max_resource_fee, expected_max_resource_fee);

        // Charged Fees
        let expected_fee_charged = 100;
        let expected_non_refundable_charged = DEFAULT_RESOURCE_FEE;
        let expected_refundable_charged = DEFAULT_RESOURCE_FEE;
        let expected_resource_fee_charged =
            expected_non_refundable_charged + expected_refundable_charged;
        let expected_inclusion_fee_charged = expected_fee_charged - expected_resource_fee_charged;

        assert!(!fee_table.should_include_resource_fees());
        assert_eq!(fee_table.fee_charged, expected_fee_charged);
        assert_eq!(
            fee_table.resource_fee_charged,
            expected_resource_fee_charged
        );
        assert_eq!(
            fee_table.non_refundable_resource_fee_charged,
            expected_non_refundable_charged
        );
        assert_eq!(
            fee_table.refundable_resource_fee_charged,
            expected_refundable_charged
        );
        assert_eq!(
            fee_table.inclusion_fee_charged,
            expected_inclusion_fee_charged
        );
    }

    #[test]
    fn fee_bump_wrapping_classic_tx_fee_table() {
        let resp = fee_bump_wrapping_classic_tx_response().unwrap();
        let fee_table = FeeTable::new_from_transaction_response(&resp).unwrap();

        // Proposed Fees
        let expected_max_fee = 400;
        let expected_max_resource_fee = DEFAULT_RESOURCE_FEE;
        assert_eq!(fee_table.max_fee, expected_max_fee);
        assert_eq!(fee_table.max_resource_fee, expected_max_resource_fee);

        // Charged Fees
        let expected_fee_charged = 200;
        let expected_non_refundable_charged = DEFAULT_RESOURCE_FEE;
        let expected_refundable_charged = DEFAULT_RESOURCE_FEE;
        let expected_resource_fee_charged =
            expected_non_refundable_charged + expected_refundable_charged;
        let expected_inclusion_fee_charged = expected_fee_charged - expected_resource_fee_charged;

        assert!(!fee_table.should_include_resource_fees());
        assert_eq!(fee_table.fee_charged, expected_fee_charged);
        assert_eq!(
            fee_table.resource_fee_charged,
            expected_resource_fee_charged
        );
        assert_eq!(
            fee_table.non_refundable_resource_fee_charged,
            expected_non_refundable_charged
        );
        assert_eq!(
            fee_table.refundable_resource_fee_charged,
            expected_refundable_charged
        );
        assert_eq!(
            fee_table.inclusion_fee_charged,
            expected_inclusion_fee_charged
        );
    }

    #[test]
    fn fee_bump_wrapping_soroban_tx_fee_table() {
        let resp = fee_bump_wrapping_soroban_tx_response().unwrap();
        let fee_table = FeeTable::new_from_transaction_response(&resp).unwrap();

        // PROPOSED FEES
        let tx_proposed_fee = 10208876;
        let inner_tx_resource_fee = 5004438;
        let inner_tx_proposed_fee = 5004538;
        let inner_tx_inclusion_fee = inner_tx_proposed_fee - inner_tx_resource_fee;

        let expected_max_fee = tx_proposed_fee;
        let expected_max_resource_fee = inner_tx_resource_fee;

        /*  the expected max inclusion fee should be the fee bump tx fee minus the following fees since they will not be charged to the fee bump tx source acct:
        - inner tx resource fee
        - inner tx inclusion fee
        */
        let expected_max_inclusion_fee =
            tx_proposed_fee - inner_tx_resource_fee - inner_tx_inclusion_fee;

        assert_eq!(fee_table.max_fee, expected_max_fee);
        assert_eq!(fee_table.max_resource_fee, expected_max_resource_fee);
        assert_eq!(fee_table.max_inclusion_fee, expected_max_inclusion_fee);

        // ACTUAL FEES
        let expected_fee_charged = 3603030;
        let expected_non_refundable_charged = 285226;
        let expected_refundable_charged = 3317604;

        let expected_resource_fee_charged =
            expected_non_refundable_charged + expected_refundable_charged;
        let expected_inclusion_fee_charged = expected_fee_charged - expected_resource_fee_charged;

        assert!(fee_table.should_include_resource_fees());
        assert_eq!(fee_table.fee_charged, expected_fee_charged);
        assert_eq!(
            fee_table.resource_fee_charged,
            expected_resource_fee_charged
        );
        assert_eq!(
            fee_table.non_refundable_resource_fee_charged,
            expected_non_refundable_charged
        );
        assert_eq!(
            fee_table.refundable_resource_fee_charged,
            expected_refundable_charged
        );
        assert_eq!(
            fee_table.inclusion_fee_charged,
            expected_inclusion_fee_charged
        );
    }

    fn soroban_tx_response() -> Result<GetTransactionResponse, serde_json::Error> {
        serde_json::from_str(SOROBAN_TX_RESPONSE)
    }

    fn classic_tx_response() -> Result<GetTransactionResponse, serde_json::Error> {
        serde_json::from_str(CLASSIC_TX_RESPONSE)
    }

    fn fee_bump_wrapping_classic_tx_response() -> Result<GetTransactionResponse, serde_json::Error>
    {
        serde_json::from_str(FEE_BUMP_WRAPPING_CLASSIC_TX_RESPONSE)
    }

    fn fee_bump_wrapping_soroban_tx_response() -> Result<GetTransactionResponse, serde_json::Error>
    {
        serde_json::from_str(FEE_BUMP_WRAPPING_SOROBAN_TX_RESPONSE)
    }

    const SOROBAN_TX_RESPONSE: &str = r#"{"status":"SUCCESS","envelope":{"tx":{"tx":{"source_account":"GDWREJ5HETNIDTQKXJZPA6LRSJMFUCO4T2DFEJYSZ2XVWRTMUG64AL4B","fee":105447,"seq_num":"2062499130114054","cond":"none","memo":"none","operations":[{"source_account":null,"body":{"invoke_host_function":{"host_function":{"invoke_contract":{"contract_address":"CCFNZO33IO6GDTPLWWRJ5F34UBXEBOSYGSQJJGVLAJNNULU26CRZR6TM","function_name":"inc","args":[]}},"auth":[]}}}],"ext":{"v1":{"ext":"v0","resources":{"footprint":{"read_only":[{"contract_data":{"contract":"CCFNZO33IO6GDTPLWWRJ5F34UBXEBOSYGSQJJGVLAJNNULU26CRZR6TM","key":"ledger_key_contract_instance","durability":"persistent"}},{"contract_code":{"hash":"2a41e16cb574fc372ee81f02c1d775365d9d39002cc630bd162a4dcaeb153161"}}],"read_write":[{"contract_data":{"contract":"CCFNZO33IO6GDTPLWWRJ5F34UBXEBOSYGSQJJGVLAJNNULU26CRZR6TM","key":{"symbol":"COUNTER"},"durability":"persistent"}}]},"instructions":2099865,"disk_read_bytes":8008,"write_bytes":80},"resource_fee":"105347"}}},"signatures":[{"hint":"6ca1bdc0","signature":"b8120310a5db9b5fd295f00d9fd7ebc26e34d14b27a00a5827aa89a18027fb7d69fb1b11e3b6408885602627b228256fde049aee2045fea7d088327302e4ea04"}]}},"result":{"fee_charged":"60537","result":{"tx_success":[{"op_inner":{"invoke_host_function":{"success":"e18456c437deb4d21dceee8db938ac8bcea25405af8df02d9225104e5d53e185"}}}]},"ext":"v0"},"result_meta":{"v3":{"ext":"v0","tx_changes_before":[{"state":{"last_modified_ledger_seq":480745,"data":{"account":{"account_id":"GDWREJ5HETNIDTQKXJZPA6LRSJMFUCO4T2DFEJYSZ2XVWRTMUG64AL4B","balance":"99907097552","seq_num":"2062499130114053","num_sub_entries":1,"inflation_dest":null,"flags":0,"home_domain":"","thresholds":"01000000","signers":[],"ext":{"v1":{"liabilities":{"buying":"0","selling":"0"},"ext":{"v2":{"num_sponsored":0,"num_sponsoring":0,"signer_sponsoring_i_ds":[],"ext":{"v3":{"ext":"v0","seq_ledger":480418,"seq_time":"1752671669"}}}}}}}},"ext":"v0"}},{"updated":{"last_modified_ledger_seq":480745,"data":{"account":{"account_id":"GDWREJ5HETNIDTQKXJZPA6LRSJMFUCO4T2DFEJYSZ2XVWRTMUG64AL4B","balance":"99907097552","seq_num":"2062499130114054","num_sub_entries":1,"inflation_dest":null,"flags":0,"home_domain":"","thresholds":"01000000","signers":[],"ext":{"v1":{"liabilities":{"buying":"0","selling":"0"},"ext":{"v2":{"num_sponsored":0,"num_sponsoring":0,"signer_sponsoring_i_ds":[],"ext":{"v3":{"ext":"v0","seq_ledger":480745,"seq_time":"1752673305"}}}}}}}},"ext":"v0"}}],"operations":[{"changes":[{"state":{"last_modified_ledger_seq":480290,"data":{"contract_data":{"ext":"v0","contract":"CCFNZO33IO6GDTPLWWRJ5F34UBXEBOSYGSQJJGVLAJNNULU26CRZR6TM","key":{"symbol":"COUNTER"},"durability":"persistent","val":{"u32":2}}},"ext":"v0"}},{"updated":{"last_modified_ledger_seq":480745,"data":{"contract_data":{"ext":"v0","contract":"CCFNZO33IO6GDTPLWWRJ5F34UBXEBOSYGSQJJGVLAJNNULU26CRZR6TM","key":{"symbol":"COUNTER"},"durability":"persistent","val":{"u32":3}}},"ext":"v0"}}]}],"tx_changes_after":[{"state":{"last_modified_ledger_seq":480745,"data":{"account":{"account_id":"GDWREJ5HETNIDTQKXJZPA6LRSJMFUCO4T2DFEJYSZ2XVWRTMUG64AL4B","balance":"99907097552","seq_num":"2062499130114054","num_sub_entries":1,"inflation_dest":null,"flags":0,"home_domain":"","thresholds":"01000000","signers":[],"ext":{"v1":{"liabilities":{"buying":"0","selling":"0"},"ext":{"v2":{"num_sponsored":0,"num_sponsoring":0,"signer_sponsoring_i_ds":[],"ext":{"v3":{"ext":"v0","seq_ledger":480745,"seq_time":"1752673305"}}}}}}}},"ext":"v0"}},{"updated":{"last_modified_ledger_seq":480745,"data":{"account":{"account_id":"GDWREJ5HETNIDTQKXJZPA6LRSJMFUCO4T2DFEJYSZ2XVWRTMUG64AL4B","balance":"99907142462","seq_num":"2062499130114054","num_sub_entries":1,"inflation_dest":null,"flags":0,"home_domain":"","thresholds":"01000000","signers":[],"ext":{"v1":{"liabilities":{"buying":"0","selling":"0"},"ext":{"v2":{"num_sponsored":0,"num_sponsoring":0,"signer_sponsoring_i_ds":[],"ext":{"v3":{"ext":"v0","seq_ledger":480745,"seq_time":"1752673305"}}}}}}}},"ext":"v0"}}],"soroban_meta":{"ext":{"v1":{"ext":"v0","total_non_refundable_resource_fee_charged":"60358","total_refundable_resource_fee_charged":"79","rent_fee_charged":"0"}},"events":[],"return_value":{"u32":3},"diagnostic_events":[{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"fn_call"},{"bytes":"8adcbb7b43bc61cdebb5a29e977ca06e40ba5834a0949aab025ada2e9af0a398"},{"symbol":"inc"}],"data":"void"}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":"CCFNZO33IO6GDTPLWWRJ5F34UBXEBOSYGSQJJGVLAJNNULU26CRZR6TM","type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"log"}],"data":{"vec":[{"string":"count: {}"},{"u32":2}]}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":"CCFNZO33IO6GDTPLWWRJ5F34UBXEBOSYGSQJJGVLAJNNULU26CRZR6TM","type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"fn_return"},{"symbol":"inc"}],"data":{"u32":3}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"read_entry"}],"data":{"u64":"6"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"write_entry"}],"data":{"u64":"1"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"ledger_read_byte"}],"data":{"u64":"8008"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"ledger_write_byte"}],"data":{"u64":"80"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"read_key_byte"}],"data":{"u64":"144"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"write_key_byte"}],"data":{"u64":"0"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"read_data_byte"}],"data":{"u64":"184"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"write_data_byte"}],"data":{"u64":"80"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"read_code_byte"}],"data":{"u64":"7824"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"write_code_byte"}],"data":{"u64":"0"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"emit_event"}],"data":{"u64":"0"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"emit_event_byte"}],"data":{"u64":"8"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"cpu_insn"}],"data":{"u64":"2006689"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"mem_byte"}],"data":{"u64":"1492062"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"invoke_time_nsecs"}],"data":{"u64":"561706"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"max_rw_key_byte"}],"data":{"u64":"60"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"max_rw_data_byte"}],"data":{"u64":"104"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"max_rw_code_byte"}],"data":{"u64":"7824"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"max_emit_event_byte"}],"data":{"u64":"0"}}}}}]}}},"events":{"contract_events":[],"diagnostic_events":[{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"fn_call"},{"bytes":"8adcbb7b43bc61cdebb5a29e977ca06e40ba5834a0949aab025ada2e9af0a398"},{"symbol":"inc"}],"data":"void"}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":"CCFNZO33IO6GDTPLWWRJ5F34UBXEBOSYGSQJJGVLAJNNULU26CRZR6TM","type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"log"}],"data":{"vec":[{"string":"count: {}"},{"u32":2}]}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":"CCFNZO33IO6GDTPLWWRJ5F34UBXEBOSYGSQJJGVLAJNNULU26CRZR6TM","type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"fn_return"},{"symbol":"inc"}],"data":{"u32":3}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"read_entry"}],"data":{"u64":"6"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"write_entry"}],"data":{"u64":"1"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"ledger_read_byte"}],"data":{"u64":"8008"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"ledger_write_byte"}],"data":{"u64":"80"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"read_key_byte"}],"data":{"u64":"144"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"write_key_byte"}],"data":{"u64":"0"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"read_data_byte"}],"data":{"u64":"184"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"write_data_byte"}],"data":{"u64":"80"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"read_code_byte"}],"data":{"u64":"7824"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"write_code_byte"}],"data":{"u64":"0"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"emit_event"}],"data":{"u64":"0"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"emit_event_byte"}],"data":{"u64":"8"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"cpu_insn"}],"data":{"u64":"2006689"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"mem_byte"}],"data":{"u64":"1492062"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"invoke_time_nsecs"}],"data":{"u64":"561706"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"max_rw_key_byte"}],"data":{"u64":"60"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"max_rw_data_byte"}],"data":{"u64":"104"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"max_rw_code_byte"}],"data":{"u64":"7824"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"max_emit_event_byte"}],"data":{"u64":"0"}}}}}],"transaction_events":[]}}"#;

    const CLASSIC_TX_RESPONSE: &str = r#"{"status":"SUCCESS","envelope":{"tx":{"tx":{"source_account":"GDWREJ5HETNIDTQKXJZPA6LRSJMFUCO4T2DFEJYSZ2XVWRTMUG64AL4B","fee":100,"seq_num":"2062499130114053","cond":"none","memo":"none","operations":[{"source_account":null,"body":{"manage_data":{"data_name":"test","data_value":"abcdef"}}}],"ext":"v0"},"signatures":[{"hint":"6ca1bdc0","signature":"a12761eee624d0a15f731b6e63201c55978d714a28d167e80441092afb11a06549056199e589ff511d376299782cde796169a1781b7ecad93cbe68ac3a768d05"}]}},"result":{"fee_charged":"100","result":{"tx_success":[{"op_inner":{"manage_data":"success"}}]},"ext":"v0"},"result_meta":{"v3":{"ext":"v0","tx_changes_before":[{"state":{"last_modified_ledger_seq":480418,"data":{"account":{"account_id":"GDWREJ5HETNIDTQKXJZPA6LRSJMFUCO4T2DFEJYSZ2XVWRTMUG64AL4B","balance":"99907202999","seq_num":"2062499130114052","num_sub_entries":0,"inflation_dest":null,"flags":0,"home_domain":"","thresholds":"01000000","signers":[],"ext":{"v1":{"liabilities":{"buying":"0","selling":"0"},"ext":{"v2":{"num_sponsored":0,"num_sponsoring":0,"signer_sponsoring_i_ds":[],"ext":{"v3":{"ext":"v0","seq_ledger":480290,"seq_time":"1752671028"}}}}}}}},"ext":"v0"}},{"updated":{"last_modified_ledger_seq":480418,"data":{"account":{"account_id":"GDWREJ5HETNIDTQKXJZPA6LRSJMFUCO4T2DFEJYSZ2XVWRTMUG64AL4B","balance":"99907202999","seq_num":"2062499130114053","num_sub_entries":0,"inflation_dest":null,"flags":0,"home_domain":"","thresholds":"01000000","signers":[],"ext":{"v1":{"liabilities":{"buying":"0","selling":"0"},"ext":{"v2":{"num_sponsored":0,"num_sponsoring":0,"signer_sponsoring_i_ds":[],"ext":{"v3":{"ext":"v0","seq_ledger":480418,"seq_time":"1752671669"}}}}}}}},"ext":"v0"}}],"operations":[{"changes":[{"created":{"last_modified_ledger_seq":480418,"data":{"data":{"account_id":"GDWREJ5HETNIDTQKXJZPA6LRSJMFUCO4T2DFEJYSZ2XVWRTMUG64AL4B","data_name":"test","data_value":"abcdef","ext":"v0"}},"ext":"v0"}},{"state":{"last_modified_ledger_seq":480418,"data":{"account":{"account_id":"GDWREJ5HETNIDTQKXJZPA6LRSJMFUCO4T2DFEJYSZ2XVWRTMUG64AL4B","balance":"99907202999","seq_num":"2062499130114053","num_sub_entries":0,"inflation_dest":null,"flags":0,"home_domain":"","thresholds":"01000000","signers":[],"ext":{"v1":{"liabilities":{"buying":"0","selling":"0"},"ext":{"v2":{"num_sponsored":0,"num_sponsoring":0,"signer_sponsoring_i_ds":[],"ext":{"v3":{"ext":"v0","seq_ledger":480418,"seq_time":"1752671669"}}}}}}}},"ext":"v0"}},{"updated":{"last_modified_ledger_seq":480418,"data":{"account":{"account_id":"GDWREJ5HETNIDTQKXJZPA6LRSJMFUCO4T2DFEJYSZ2XVWRTMUG64AL4B","balance":"99907202999","seq_num":"2062499130114053","num_sub_entries":1,"inflation_dest":null,"flags":0,"home_domain":"","thresholds":"01000000","signers":[],"ext":{"v1":{"liabilities":{"buying":"0","selling":"0"},"ext":{"v2":{"num_sponsored":0,"num_sponsoring":0,"signer_sponsoring_i_ds":[],"ext":{"v3":{"ext":"v0","seq_ledger":480418,"seq_time":"1752671669"}}}}}}}},"ext":"v0"}}]}],"tx_changes_after":[],"soroban_meta":null}},"events":{"contract_events":[],"diagnostic_events":[],"transaction_events":[]}}"#;

    const FEE_BUMP_WRAPPING_CLASSIC_TX_RESPONSE: &str = r#"{"status":"SUCCESS","envelope":{"tx_fee_bump":{"tx":{"fee_source":"GD5EJEGJM5PWKZ4WBJFMTHHY3VNUDJDU55N24ODPIPNYKBRRJCCIA44P","fee":"400","inner_tx":{"tx":{"tx":{"source_account":"GDWREJ5HETNIDTQKXJZPA6LRSJMFUCO4T2DFEJYSZ2XVWRTMUG64AL4B","fee":100,"seq_num":"2062499130114055","cond":{"time":{"min_time":"0","max_time":"1752675654"}},"memo":"none","operations":[{"source_account":null,"body":{"payment":{"destination":"GDBFMEGF2EVTNISNTYVOOYGXAEP5A353YJCPDRGUH3L6GMIDATR4BWY6","asset":"native","amount":"100000000"}}}],"ext":"v0"},"signatures":[{"hint":"6ca1bdc0","signature":"ce5f19bac1e1a57f6f54a7d4f5729fd2db8755dac425fd61220111e3d4436dfd52e9f0f0098ea0c07fc3e65c69f19f7d1f440adc7fa9937662bd9268fb7cb00c"}]}},"ext":"v0"},"signatures":[{"hint":"31488480","signature":"789b8261c481532c7f8933ed1b32d9fb270d9acc044774dda1986f20aba8248592975f25eec1aabe374978fcc10a19b9797c834d686465a4d225b01d0c57020e"}]}},"result":{"fee_charged":"200","result":{"tx_fee_bump_inner_success":{"transaction_hash":"b6b9591c8c00d1aa9212ef0345e6b1ccd56f9a362e463a1f6237423d09dbcab8","result":{"fee_charged":"100","result":{"tx_success":[{"op_inner":{"payment":"success"}}]},"ext":"v0"}}},"ext":"v0"},"result_meta":{"v3":{"ext":"v0","tx_changes_before":[{"state":{"last_modified_ledger_seq":481209,"data":{"account":{"account_id":"GD5EJEGJM5PWKZ4WBJFMTHHY3VNUDJDU55N24ODPIPNYKBRRJCCIA44P","balance":"99999999800","seq_num":"2065887859310592","num_sub_entries":0,"inflation_dest":null,"flags":0,"home_domain":"","thresholds":"01000000","signers":[],"ext":"v0"}},"ext":"v0"}},{"updated":{"last_modified_ledger_seq":481209,"data":{"account":{"account_id":"GD5EJEGJM5PWKZ4WBJFMTHHY3VNUDJDU55N24ODPIPNYKBRRJCCIA44P","balance":"99999999800","seq_num":"2065887859310592","num_sub_entries":0,"inflation_dest":null,"flags":0,"home_domain":"","thresholds":"01000000","signers":[],"ext":"v0"}},"ext":"v0"}},{"state":{"last_modified_ledger_seq":481027,"data":{"account":{"account_id":"GDWREJ5HETNIDTQKXJZPA6LRSJMFUCO4T2DFEJYSZ2XVWRTMUG64AL4B","balance":"199907142462","seq_num":"2062499130114054","num_sub_entries":1,"inflation_dest":null,"flags":0,"home_domain":"","thresholds":"01000000","signers":[],"ext":{"v1":{"liabilities":{"buying":"0","selling":"0"},"ext":{"v2":{"num_sponsored":0,"num_sponsoring":0,"signer_sponsoring_i_ds":[],"ext":{"v3":{"ext":"v0","seq_ledger":480745,"seq_time":"1752673305"}}}}}}}},"ext":"v0"}},{"updated":{"last_modified_ledger_seq":481209,"data":{"account":{"account_id":"GDWREJ5HETNIDTQKXJZPA6LRSJMFUCO4T2DFEJYSZ2XVWRTMUG64AL4B","balance":"199907142462","seq_num":"2062499130114055","num_sub_entries":1,"inflation_dest":null,"flags":0,"home_domain":"","thresholds":"01000000","signers":[],"ext":{"v1":{"liabilities":{"buying":"0","selling":"0"},"ext":{"v2":{"num_sponsored":0,"num_sponsoring":0,"signer_sponsoring_i_ds":[],"ext":{"v3":{"ext":"v0","seq_ledger":481209,"seq_time":"1752675627"}}}}}}}},"ext":"v0"}}],"operations":[{"changes":[{"state":{"last_modified_ledger_seq":481209,"data":{"account":{"account_id":"GDWREJ5HETNIDTQKXJZPA6LRSJMFUCO4T2DFEJYSZ2XVWRTMUG64AL4B","balance":"199907142462","seq_num":"2062499130114055","num_sub_entries":1,"inflation_dest":null,"flags":0,"home_domain":"","thresholds":"01000000","signers":[],"ext":{"v1":{"liabilities":{"buying":"0","selling":"0"},"ext":{"v2":{"num_sponsored":0,"num_sponsoring":0,"signer_sponsoring_i_ds":[],"ext":{"v3":{"ext":"v0","seq_ledger":481209,"seq_time":"1752675627"}}}}}}}},"ext":"v0"}},{"updated":{"last_modified_ledger_seq":481209,"data":{"account":{"account_id":"GDWREJ5HETNIDTQKXJZPA6LRSJMFUCO4T2DFEJYSZ2XVWRTMUG64AL4B","balance":"199807142462","seq_num":"2062499130114055","num_sub_entries":1,"inflation_dest":null,"flags":0,"home_domain":"","thresholds":"01000000","signers":[],"ext":{"v1":{"liabilities":{"buying":"0","selling":"0"},"ext":{"v2":{"num_sponsored":0,"num_sponsoring":0,"signer_sponsoring_i_ds":[],"ext":{"v3":{"ext":"v0","seq_ledger":481209,"seq_time":"1752675627"}}}}}}}},"ext":"v0"}},{"state":{"last_modified_ledger_seq":481014,"data":{"account":{"account_id":"GDBFMEGF2EVTNISNTYVOOYGXAEP5A353YJCPDRGUH3L6GMIDATR4BWY6","balance":"100000000000","seq_num":"2065939398918144","num_sub_entries":0,"inflation_dest":null,"flags":0,"home_domain":"","thresholds":"01000000","signers":[],"ext":"v0"}},"ext":"v0"}},{"updated":{"last_modified_ledger_seq":481209,"data":{"account":{"account_id":"GDBFMEGF2EVTNISNTYVOOYGXAEP5A353YJCPDRGUH3L6GMIDATR4BWY6","balance":"100100000000","seq_num":"2065939398918144","num_sub_entries":0,"inflation_dest":null,"flags":0,"home_domain":"","thresholds":"01000000","signers":[],"ext":"v0"}},"ext":"v0"}}]}],"tx_changes_after":[],"soroban_meta":null}},"events":{"contract_events":[],"diagnostic_events":[],"transaction_events":[]}}"#;

    const FEE_BUMP_WRAPPING_SOROBAN_TX_RESPONSE: &str = r#"{"status":"SUCCESS","envelope":{"tx_fee_bump":{"tx":{"fee_source":"GDJLH2F7DBI6GC22J7YUTPAEFRSWKG5MN5RSE2GOOYUTO4BH66LHENRW","fee":"10208876","inner_tx":{"tx":{"tx":{"source_account":"GBQUFZ3QRIP6VQ74BV6KJGBEJ7YFE4WGRCB4YCMGTXFEMYLXNI2CC2AK","fee":5004538,"seq_num":"244204891193475075","cond":"none","memo":"none","operations":[{"source_account":null,"body":{"invoke_host_function":{"host_function":{"invoke_contract":{"contract_address":"CDVQVKOY2YSXS2IC7KN6MNASSHPAO7UN2UR2ON4OI2SKMFJNVAMDX6DP","function_name":"submit","args":[{"address":"GBQUFZ3QRIP6VQ74BV6KJGBEJ7YFE4WGRCB4YCMGTXFEMYLXNI2CC2AK"},{"address":"GBQUFZ3QRIP6VQ74BV6KJGBEJ7YFE4WGRCB4YCMGTXFEMYLXNI2CC2AK"},{"address":"GBQUFZ3QRIP6VQ74BV6KJGBEJ7YFE4WGRCB4YCMGTXFEMYLXNI2CC2AK"},{"vec":[{"map":[{"key":{"symbol":"address"},"val":{"address":"CCW67TSZV3SSS2HXMBQ5JFGCKJNXKZM7UQUWUZPUTHXSTZLEO7SJMI75"}},{"key":{"symbol":"amount"},"val":{"i128":"10000990"}},{"key":{"symbol":"request_type"},"val":{"u32":3}}]}]}]}},"auth":[{"credentials":"source_account","root_invocation":{"function":{"contract_fn":{"contract_address":"CDVQVKOY2YSXS2IC7KN6MNASSHPAO7UN2UR2ON4OI2SKMFJNVAMDX6DP","function_name":"submit","args":[{"address":"GBQUFZ3QRIP6VQ74BV6KJGBEJ7YFE4WGRCB4YCMGTXFEMYLXNI2CC2AK"},{"address":"GBQUFZ3QRIP6VQ74BV6KJGBEJ7YFE4WGRCB4YCMGTXFEMYLXNI2CC2AK"},{"address":"GBQUFZ3QRIP6VQ74BV6KJGBEJ7YFE4WGRCB4YCMGTXFEMYLXNI2CC2AK"},{"vec":[{"map":[{"key":{"symbol":"address"},"val":{"address":"CCW67TSZV3SSS2HXMBQ5JFGCKJNXKZM7UQUWUZPUTHXSTZLEO7SJMI75"}},{"key":{"symbol":"amount"},"val":{"i128":"10000990"}},{"key":{"symbol":"request_type"},"val":{"u32":3}}]}]}]}},"sub_invocations":[]}}]}}}],"ext":{"v1":{"ext":"v0","resources":{"footprint":{"read_only":[{"contract_data":{"contract":"CCW67TSZV3SSS2HXMBQ5JFGCKJNXKZM7UQUWUZPUTHXSTZLEO7SJMI75","key":"ledger_key_contract_instance","durability":"persistent"}},{"contract_data":{"contract":"CDVQVKOY2YSXS2IC7KN6MNASSHPAO7UN2UR2ON4OI2SKMFJNVAMDX6DP","key":{"vec":[{"symbol":"EmisConfig"},{"u32":3}]},"durability":"persistent"}},{"contract_data":{"contract":"CDVQVKOY2YSXS2IC7KN6MNASSHPAO7UN2UR2ON4OI2SKMFJNVAMDX6DP","key":{"vec":[{"symbol":"EmisData"},{"u32":3}]},"durability":"persistent"}},{"contract_data":{"contract":"CDVQVKOY2YSXS2IC7KN6MNASSHPAO7UN2UR2ON4OI2SKMFJNVAMDX6DP","key":{"vec":[{"symbol":"ResConfig"},{"address":"CCW67TSZV3SSS2HXMBQ5JFGCKJNXKZM7UQUWUZPUTHXSTZLEO7SJMI75"}]},"durability":"persistent"}},{"contract_data":{"contract":"CDVQVKOY2YSXS2IC7KN6MNASSHPAO7UN2UR2ON4OI2SKMFJNVAMDX6DP","key":"ledger_key_contract_instance","durability":"persistent"}},{"contract_code":{"hash":"baf978f10efdbcd85747868bef8832845ea6809f7643b67a4ac0cd669327fc2c"}}],"read_write":[{"trustline":{"account_id":"GBQUFZ3QRIP6VQ74BV6KJGBEJ7YFE4WGRCB4YCMGTXFEMYLXNI2CC2AK","asset":{"credit_alphanum4":{"asset_code":"USDC","issuer":"GA5ZSEJYB37JRC5AVCIA5MOP4RHTM335X2KGX3IHOJAPP5RE34K4KZVN"}}}},{"contract_data":{"contract":"CCW67TSZV3SSS2HXMBQ5JFGCKJNXKZM7UQUWUZPUTHXSTZLEO7SJMI75","key":{"vec":[{"symbol":"Balance"},{"address":"CDVQVKOY2YSXS2IC7KN6MNASSHPAO7UN2UR2ON4OI2SKMFJNVAMDX6DP"}]},"durability":"persistent"}},{"contract_data":{"contract":"CDVQVKOY2YSXS2IC7KN6MNASSHPAO7UN2UR2ON4OI2SKMFJNVAMDX6DP","key":{"vec":[{"symbol":"Positions"},{"address":"GBQUFZ3QRIP6VQ74BV6KJGBEJ7YFE4WGRCB4YCMGTXFEMYLXNI2CC2AK"}]},"durability":"persistent"}},{"contract_data":{"contract":"CDVQVKOY2YSXS2IC7KN6MNASSHPAO7UN2UR2ON4OI2SKMFJNVAMDX6DP","key":{"vec":[{"symbol":"ResData"},{"address":"CCW67TSZV3SSS2HXMBQ5JFGCKJNXKZM7UQUWUZPUTHXSTZLEO7SJMI75"}]},"durability":"persistent"}},{"contract_data":{"contract":"CDVQVKOY2YSXS2IC7KN6MNASSHPAO7UN2UR2ON4OI2SKMFJNVAMDX6DP","key":{"vec":[{"symbol":"UserEmis"},{"map":[{"key":{"symbol":"reserve_id"},"val":{"u32":3}},{"key":{"symbol":"user"},"val":{"address":"GBQUFZ3QRIP6VQ74BV6KJGBEJ7YFE4WGRCB4YCMGTXFEMYLXNI2CC2AK"}}]}]},"durability":"persistent"}}]},"instructions":9237256,"disk_read_bytes":53060,"write_bytes":1276},"resource_fee":"5004438"}}},"signatures":[{"hint":"f291f849","signature":"3cc2da7de9df730b23ffa8a26ddfe180aa4b8eef3e251c6f05972984a659a729f1cea6396c806cce57dc66d070e588708021dfd6dc510454355bd9ce3be55500"}]}},"ext":"v0"},"signatures":[{"hint":"27f79672","signature":"f5985e8d0d8d1acc1e9862418ad09da9ec9607327362a887ee8fd805d362bcd97dee470c8f604cf5e4acbf161be4e79ece729333ba39f126d64781ecbe763202"}]}},"result":{"fee_charged":"3603030","result":{"tx_fee_bump_inner_success":{"transaction_hash":"0d2bdcf1532b215a81730267d6a7cd444127b19bdb435a568543890951a95d78","result":{"fee_charged":"3602930","result":{"tx_success":[{"op_inner":{"invoke_host_function":{"success":"1437b07cfee492dc5c26ccebe96fcab3c8a96b9a0e29d2b804095d6cc8e2f89d"}}}]},"ext":"v0"}}},"ext":"v0"},"result_meta":{"v3":{"ext":"v0","tx_changes_before":[{"state":{"last_modified_ledger_seq":58166971,"data":{"account":{"account_id":"GDJLH2F7DBI6GC22J7YUTPAEFRSWKG5MN5RSE2GOOYUTO4BH66LHENRW","balance":"22097264303","seq_num":"181263292226863151","num_sub_entries":4,"inflation_dest":null,"flags":0,"home_domain":"","thresholds":"01000000","signers":[],"ext":{"v1":{"liabilities":{"buying":"0","selling":"0"},"ext":{"v2":{"num_sponsored":0,"num_sponsoring":2,"signer_sponsoring_i_ds":[],"ext":{"v3":{"ext":"v0","seq_ledger":56604244,"seq_time":"1744587782"}}}}}}}},"ext":"v0"}},{"updated":{"last_modified_ledger_seq":58166971,"data":{"account":{"account_id":"GDJLH2F7DBI6GC22J7YUTPAEFRSWKG5MN5RSE2GOOYUTO4BH66LHENRW","balance":"22097264303","seq_num":"181263292226863151","num_sub_entries":4,"inflation_dest":null,"flags":0,"home_domain":"","thresholds":"01000000","signers":[],"ext":{"v1":{"liabilities":{"buying":"0","selling":"0"},"ext":{"v2":{"num_sponsored":0,"num_sponsoring":2,"signer_sponsoring_i_ds":[],"ext":{"v3":{"ext":"v0","seq_ledger":56604244,"seq_time":"1744587782"}}}}}}}},"ext":"v0"}},{"state":{"last_modified_ledger_seq":56858638,"data":{"account":{"account_id":"GBQUFZ3QRIP6VQ74BV6KJGBEJ7YFE4WGRCB4YCMGTXFEMYLXNI2CC2AK","balance":"0","seq_num":"244204891193475074","num_sub_entries":4,"inflation_dest":null,"flags":0,"home_domain":"","thresholds":"00141414","signers":[{"key":"GCO6RBY3BSJ77Y77TUXV2O5AV5E5WHAKRVMFDBHHX4H4KSKPVMICFKT4","weight":10},{"key":"GDG2THNO7333WXJU2ZMFAIDYEMJHWLHZLAJ6ZEV2QPWPWT7SSH4ETPIW","weight":20},{"key":"GDRWVPEIZK3YDKSLFPY4I4S2FOFZ6SJIRTUHTFN4NZGZTZGOIBRD4CT7","weight":10}],"ext":{"v1":{"liabilities":{"buying":"0","selling":"0"},"ext":{"v2":{"num_sponsored":6,"num_sponsoring":0,"signer_sponsoring_i_ds":["GBKAZKU33LRJX47UGDX2YGA7UIJ5BWSVAFQJLBAUYIMOS5KBPVXKGO4X","GCUISJEWU2TZ4QIJNGNVU4BSZ5CQS3KE6A3N3ETOV7XHCBVO4GLTLGOQ","GBKAZKU33LRJX47UGDX2YGA7UIJ5BWSVAFQJLBAUYIMOS5KBPVXKGO4X"],"ext":{"v3":{"ext":"v0","seq_ledger":56858638,"seq_time":"1746041747"}}}}}}}},"ext":{"v1":{"sponsoring_id":"GBKAZKU33LRJX47UGDX2YGA7UIJ5BWSVAFQJLBAUYIMOS5KBPVXKGO4X","ext":"v0"}}}},{"updated":{"last_modified_ledger_seq":58166971,"data":{"account":{"account_id":"GBQUFZ3QRIP6VQ74BV6KJGBEJ7YFE4WGRCB4YCMGTXFEMYLXNI2CC2AK","balance":"0","seq_num":"244204891193475075","num_sub_entries":4,"inflation_dest":null,"flags":0,"home_domain":"","thresholds":"00141414","signers":[{"key":"GCO6RBY3BSJ77Y77TUXV2O5AV5E5WHAKRVMFDBHHX4H4KSKPVMICFKT4","weight":10},{"key":"GDG2THNO7333WXJU2ZMFAIDYEMJHWLHZLAJ6ZEV2QPWPWT7SSH4ETPIW","weight":20},{"key":"GDRWVPEIZK3YDKSLFPY4I4S2FOFZ6SJIRTUHTFN4NZGZTZGOIBRD4CT7","weight":10}],"ext":{"v1":{"liabilities":{"buying":"0","selling":"0"},"ext":{"v2":{"num_sponsored":6,"num_sponsoring":0,"signer_sponsoring_i_ds":["GBKAZKU33LRJX47UGDX2YGA7UIJ5BWSVAFQJLBAUYIMOS5KBPVXKGO4X","GCUISJEWU2TZ4QIJNGNVU4BSZ5CQS3KE6A3N3ETOV7XHCBVO4GLTLGOQ","GBKAZKU33LRJX47UGDX2YGA7UIJ5BWSVAFQJLBAUYIMOS5KBPVXKGO4X"],"ext":{"v3":{"ext":"v0","seq_ledger":58166971,"seq_time":"1753467627"}}}}}}}},"ext":{"v1":{"sponsoring_id":"GBKAZKU33LRJX47UGDX2YGA7UIJ5BWSVAFQJLBAUYIMOS5KBPVXKGO4X","ext":"v0"}}}}],"operations":[{"changes":[{"state":{"last_modified_ledger_seq":56858638,"data":{"contract_data":{"ext":"v0","contract":"CDVQVKOY2YSXS2IC7KN6MNASSHPAO7UN2UR2ON4OI2SKMFJNVAMDX6DP","key":{"vec":[{"symbol":"UserEmis"},{"map":[{"key":{"symbol":"reserve_id"},"val":{"u32":3}},{"key":{"symbol":"user"},"val":{"address":"GBQUFZ3QRIP6VQ74BV6KJGBEJ7YFE4WGRCB4YCMGTXFEMYLXNI2CC2AK"}}]}]},"durability":"persistent","val":{"map":[{"key":{"symbol":"accrued"},"val":{"i128":"0"}},{"key":{"symbol":"index"},"val":{"i128":"16117732"}}]}}},"ext":"v0"}},{"updated":{"last_modified_ledger_seq":58166971,"data":{"contract_data":{"ext":"v0","contract":"CDVQVKOY2YSXS2IC7KN6MNASSHPAO7UN2UR2ON4OI2SKMFJNVAMDX6DP","key":{"vec":[{"symbol":"UserEmis"},{"map":[{"key":{"symbol":"reserve_id"},"val":{"u32":3}},{"key":{"symbol":"user"},"val":{"address":"GBQUFZ3QRIP6VQ74BV6KJGBEJ7YFE4WGRCB4YCMGTXFEMYLXNI2CC2AK"}}]}]},"durability":"persistent","val":{"map":[{"key":{"symbol":"accrued"},"val":{"i128":"3595324"}},{"key":{"symbol":"index"},"val":{"i128":"20142282"}}]}}},"ext":"v0"}},{"state":{"last_modified_ledger_seq":58166528,"data":{"contract_data":{"ext":"v0","contract":"CDVQVKOY2YSXS2IC7KN6MNASSHPAO7UN2UR2ON4OI2SKMFJNVAMDX6DP","key":{"vec":[{"symbol":"ResData"},{"address":"CCW67TSZV3SSS2HXMBQ5JFGCKJNXKZM7UQUWUZPUTHXSTZLEO7SJMI75"}]},"durability":"persistent","val":{"map":[{"key":{"symbol":"b_rate"},"val":{"i128":"1119495346"}},{"key":{"symbol":"b_supply"},"val":{"i128":"650408667001"}},{"key":{"symbol":"backstop_credit"},"val":{"i128":"1347654276"}},{"key":{"symbol":"d_rate"},"val":{"i128":"1190401998"}},{"key":{"symbol":"d_supply"},"val":{"i128":"58684906655"}},{"key":{"symbol":"ir_mod"},"val":{"i128":"100000000"}},{"key":{"symbol":"last_time"},"val":{"u64":"1753465101"}}]}}},"ext":"v0"}},{"updated":{"last_modified_ledger_seq":58166971,"data":{"contract_data":{"ext":"v0","contract":"CDVQVKOY2YSXS2IC7KN6MNASSHPAO7UN2UR2ON4OI2SKMFJNVAMDX6DP","key":{"vec":[{"symbol":"ResData"},{"address":"CCW67TSZV3SSS2HXMBQ5JFGCKJNXKZM7UQUWUZPUTHXSTZLEO7SJMI75"}]},"durability":"persistent","val":{"map":[{"key":{"symbol":"b_rate"},"val":{"i128":"1119495371"}},{"key":{"symbol":"b_supply"},"val":{"i128":"650399733520"}},{"key":{"symbol":"backstop_credit"},"val":{"i128":"1347658442"}},{"key":{"symbol":"d_rate"},"val":{"i128":"1190402353"}},{"key":{"symbol":"d_supply"},"val":{"i128":"58684906655"}},{"key":{"symbol":"ir_mod"},"val":{"i128":"100000000"}},{"key":{"symbol":"last_time"},"val":{"u64":"1753467627"}}]}}},"ext":"v0"}},{"state":{"last_modified_ledger_seq":56858638,"data":{"ttl":{"key_hash":"ec31d93e482c805046d62dd73b28cca317660a98f88c191a59004c4c3f3f4445","live_until_ledger_seq":58932237}},"ext":"v0"}},{"updated":{"last_modified_ledger_seq":58166971,"data":{"ttl":{"key_hash":"ec31d93e482c805046d62dd73b28cca317660a98f88c191a59004c4c3f3f4445","live_until_ledger_seq":60240571}},"ext":"v0"}},{"state":{"last_modified_ledger_seq":56858638,"data":{"contract_data":{"ext":"v0","contract":"CDVQVKOY2YSXS2IC7KN6MNASSHPAO7UN2UR2ON4OI2SKMFJNVAMDX6DP","key":{"vec":[{"symbol":"Positions"},{"address":"GBQUFZ3QRIP6VQ74BV6KJGBEJ7YFE4WGRCB4YCMGTXFEMYLXNI2CC2AK"}]},"durability":"persistent","val":{"map":[{"key":{"symbol":"collateral"},"val":{"map":[{"key":{"u32":1},"val":{"i128":"8933481"}}]}},{"key":{"symbol":"liabilities"},"val":{"map":[]}},{"key":{"symbol":"supply"},"val":{"map":[]}}]}}},"ext":"v0"}},{"updated":{"last_modified_ledger_seq":58166971,"data":{"contract_data":{"ext":"v0","contract":"CDVQVKOY2YSXS2IC7KN6MNASSHPAO7UN2UR2ON4OI2SKMFJNVAMDX6DP","key":{"vec":[{"symbol":"Positions"},{"address":"GBQUFZ3QRIP6VQ74BV6KJGBEJ7YFE4WGRCB4YCMGTXFEMYLXNI2CC2AK"}]},"durability":"persistent","val":{"map":[{"key":{"symbol":"collateral"},"val":{"map":[]}},{"key":{"symbol":"liabilities"},"val":{"map":[]}},{"key":{"symbol":"supply"},"val":{"map":[]}}]}}},"ext":"v0"}},{"state":{"last_modified_ledger_seq":56858638,"data":{"ttl":{"key_hash":"23de831fb42c10fd3e52d2e5273666cc1ae375c7409df2c8c18c8dcbcebbc1d7","live_until_ledger_seq":58932237}},"ext":"v0"}},{"updated":{"last_modified_ledger_seq":58166971,"data":{"ttl":{"key_hash":"23de831fb42c10fd3e52d2e5273666cc1ae375c7409df2c8c18c8dcbcebbc1d7","live_until_ledger_seq":60240571}},"ext":"v0"}},{"state":{"last_modified_ledger_seq":58166528,"data":{"contract_data":{"ext":"v0","contract":"CCW67TSZV3SSS2HXMBQ5JFGCKJNXKZM7UQUWUZPUTHXSTZLEO7SJMI75","key":{"vec":[{"symbol":"Balance"},{"address":"CDVQVKOY2YSXS2IC7KN6MNASSHPAO7UN2UR2ON4OI2SKMFJNVAMDX6DP"}]},"durability":"persistent","val":{"map":[{"key":{"symbol":"amount"},"val":{"i128":"660267264555"}},{"key":{"symbol":"authorized"},"val":{"bool":true}},{"key":{"symbol":"clawback"},"val":{"bool":false}}]}}},"ext":"v0"}},{"updated":{"last_modified_ledger_seq":58166971,"data":{"contract_data":{"ext":"v0","contract":"CCW67TSZV3SSS2HXMBQ5JFGCKJNXKZM7UQUWUZPUTHXSTZLEO7SJMI75","key":{"vec":[{"symbol":"Balance"},{"address":"CDVQVKOY2YSXS2IC7KN6MNASSHPAO7UN2UR2ON4OI2SKMFJNVAMDX6DP"}]},"durability":"persistent","val":{"map":[{"key":{"symbol":"amount"},"val":{"i128":"660257263565"}},{"key":{"symbol":"authorized"},"val":{"bool":true}},{"key":{"symbol":"clawback"},"val":{"bool":false}}]}}},"ext":"v0"}},{"state":{"last_modified_ledger_seq":56858638,"data":{"trustline":{"account_id":"GBQUFZ3QRIP6VQ74BV6KJGBEJ7YFE4WGRCB4YCMGTXFEMYLXNI2CC2AK","asset":{"credit_alphanum4":{"asset_code":"USDC","issuer":"GA5ZSEJYB37JRC5AVCIA5MOP4RHTM335X2KGX3IHOJAPP5RE34K4KZVN"}},"balance":"0","limit":"9223372036854775807","flags":1,"ext":"v0"}},"ext":{"v1":{"sponsoring_id":"GBKAZKU33LRJX47UGDX2YGA7UIJ5BWSVAFQJLBAUYIMOS5KBPVXKGO4X","ext":"v0"}}}},{"updated":{"last_modified_ledger_seq":58166971,"data":{"trustline":{"account_id":"GBQUFZ3QRIP6VQ74BV6KJGBEJ7YFE4WGRCB4YCMGTXFEMYLXNI2CC2AK","asset":{"credit_alphanum4":{"asset_code":"USDC","issuer":"GA5ZSEJYB37JRC5AVCIA5MOP4RHTM335X2KGX3IHOJAPP5RE34K4KZVN"}},"balance":"10000990","limit":"9223372036854775807","flags":1,"ext":"v0"}},"ext":{"v1":{"sponsoring_id":"GBKAZKU33LRJX47UGDX2YGA7UIJ5BWSVAFQJLBAUYIMOS5KBPVXKGO4X","ext":"v0"}}}}]}],"tx_changes_after":[{"state":{"last_modified_ledger_seq":58166971,"data":{"account":{"account_id":"GDJLH2F7DBI6GC22J7YUTPAEFRSWKG5MN5RSE2GOOYUTO4BH66LHENRW","balance":"22097264303","seq_num":"181263292226863151","num_sub_entries":4,"inflation_dest":null,"flags":0,"home_domain":"","thresholds":"01000000","signers":[],"ext":{"v1":{"liabilities":{"buying":"0","selling":"0"},"ext":{"v2":{"num_sponsored":0,"num_sponsoring":2,"signer_sponsoring_i_ds":[],"ext":{"v3":{"ext":"v0","seq_ledger":56604244,"seq_time":"1744587782"}}}}}}}},"ext":"v0"}},{"updated":{"last_modified_ledger_seq":58166971,"data":{"account":{"account_id":"GDJLH2F7DBI6GC22J7YUTPAEFRSWKG5MN5RSE2GOOYUTO4BH66LHENRW","balance":"22098665911","seq_num":"181263292226863151","num_sub_entries":4,"inflation_dest":null,"flags":0,"home_domain":"","thresholds":"01000000","signers":[],"ext":{"v1":{"liabilities":{"buying":"0","selling":"0"},"ext":{"v2":{"num_sponsored":0,"num_sponsoring":2,"signer_sponsoring_i_ds":[],"ext":{"v3":{"ext":"v0","seq_ledger":56604244,"seq_time":"1744587782"}}}}}}}},"ext":"v0"}}],"soroban_meta":{"ext":{"v1":{"ext":"v0","total_non_refundable_resource_fee_charged":"285226","total_refundable_resource_fee_charged":"3317604","rent_fee_charged":"3312096"}},"events":[{"ext":"v0","contract_id":"CDVQVKOY2YSXS2IC7KN6MNASSHPAO7UN2UR2ON4OI2SKMFJNVAMDX6DP","type_":"contract","body":{"v0":{"topics":[{"symbol":"withdraw_collateral"},{"address":"CCW67TSZV3SSS2HXMBQ5JFGCKJNXKZM7UQUWUZPUTHXSTZLEO7SJMI75"},{"address":"GBQUFZ3QRIP6VQ74BV6KJGBEJ7YFE4WGRCB4YCMGTXFEMYLXNI2CC2AK"}],"data":{"vec":[{"i128":"10000990"},{"i128":"8933481"}]}}}},{"ext":"v0","contract_id":"CCW67TSZV3SSS2HXMBQ5JFGCKJNXKZM7UQUWUZPUTHXSTZLEO7SJMI75","type_":"contract","body":{"v0":{"topics":[{"symbol":"transfer"},{"address":"CDVQVKOY2YSXS2IC7KN6MNASSHPAO7UN2UR2ON4OI2SKMFJNVAMDX6DP"},{"address":"GBQUFZ3QRIP6VQ74BV6KJGBEJ7YFE4WGRCB4YCMGTXFEMYLXNI2CC2AK"},{"string":"USDC:GA5ZSEJYB37JRC5AVCIA5MOP4RHTM335X2KGX3IHOJAPP5RE34K4KZVN"}],"data":{"i128":"10000990"}}}}],"return_value":{"map":[{"key":{"symbol":"collateral"},"val":{"map":[]}},{"key":{"symbol":"liabilities"},"val":{"map":[]}},{"key":{"symbol":"supply"},"val":{"map":[]}}]},"diagnostic_events":[{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"fn_call"},{"bytes":"eb0aa9d8d625796902fa9be6341291de077e8dd523a7378e46a4a6152da8183b"},{"symbol":"submit"}],"data":{"vec":[{"address":"GBQUFZ3QRIP6VQ74BV6KJGBEJ7YFE4WGRCB4YCMGTXFEMYLXNI2CC2AK"},{"address":"GBQUFZ3QRIP6VQ74BV6KJGBEJ7YFE4WGRCB4YCMGTXFEMYLXNI2CC2AK"},{"address":"GBQUFZ3QRIP6VQ74BV6KJGBEJ7YFE4WGRCB4YCMGTXFEMYLXNI2CC2AK"},{"vec":[{"map":[{"key":{"symbol":"address"},"val":{"address":"CCW67TSZV3SSS2HXMBQ5JFGCKJNXKZM7UQUWUZPUTHXSTZLEO7SJMI75"}},{"key":{"symbol":"amount"},"val":{"i128":"10000990"}},{"key":{"symbol":"request_type"},"val":{"u32":3}}]}]}]}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":"CDVQVKOY2YSXS2IC7KN6MNASSHPAO7UN2UR2ON4OI2SKMFJNVAMDX6DP","type_":"contract","body":{"v0":{"topics":[{"symbol":"withdraw_collateral"},{"address":"CCW67TSZV3SSS2HXMBQ5JFGCKJNXKZM7UQUWUZPUTHXSTZLEO7SJMI75"},{"address":"GBQUFZ3QRIP6VQ74BV6KJGBEJ7YFE4WGRCB4YCMGTXFEMYLXNI2CC2AK"}],"data":{"vec":[{"i128":"10000990"},{"i128":"8933481"}]}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":"CDVQVKOY2YSXS2IC7KN6MNASSHPAO7UN2UR2ON4OI2SKMFJNVAMDX6DP","type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"fn_call"},{"bytes":"adefce59aee52968f76061d494c2525b75659fa4296a65f499ef29e56477e496"},{"symbol":"transfer"}],"data":{"vec":[{"address":"CDVQVKOY2YSXS2IC7KN6MNASSHPAO7UN2UR2ON4OI2SKMFJNVAMDX6DP"},{"address":"GBQUFZ3QRIP6VQ74BV6KJGBEJ7YFE4WGRCB4YCMGTXFEMYLXNI2CC2AK"},{"i128":"10000990"}]}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":"CCW67TSZV3SSS2HXMBQ5JFGCKJNXKZM7UQUWUZPUTHXSTZLEO7SJMI75","type_":"contract","body":{"v0":{"topics":[{"symbol":"transfer"},{"address":"CDVQVKOY2YSXS2IC7KN6MNASSHPAO7UN2UR2ON4OI2SKMFJNVAMDX6DP"},{"address":"GBQUFZ3QRIP6VQ74BV6KJGBEJ7YFE4WGRCB4YCMGTXFEMYLXNI2CC2AK"},{"string":"USDC:GA5ZSEJYB37JRC5AVCIA5MOP4RHTM335X2KGX3IHOJAPP5RE34K4KZVN"}],"data":{"i128":"10000990"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":"CCW67TSZV3SSS2HXMBQ5JFGCKJNXKZM7UQUWUZPUTHXSTZLEO7SJMI75","type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"fn_return"},{"symbol":"transfer"}],"data":"void"}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":"CDVQVKOY2YSXS2IC7KN6MNASSHPAO7UN2UR2ON4OI2SKMFJNVAMDX6DP","type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"fn_return"},{"symbol":"submit"}],"data":{"map":[{"key":{"symbol":"collateral"},"val":{"map":[]}},{"key":{"symbol":"liabilities"},"val":{"map":[]}},{"key":{"symbol":"supply"},"val":{"map":[]}}]}}}}},{"in_successful_contract_call":false,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"read_entry"}],"data":{"u64":"11"}}}}},{"in_successful_contract_call":false,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"write_entry"}],"data":{"u64":"5"}}}}},{"in_successful_contract_call":false,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"ledger_read_byte"}],"data":{"u64":"53060"}}}}},{"in_successful_contract_call":false,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"ledger_write_byte"}],"data":{"u64":"1276"}}}}},{"in_successful_contract_call":false,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"read_key_byte"}],"data":{"u64":"1008"}}}}},{"in_successful_contract_call":false,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"write_key_byte"}],"data":{"u64":"0"}}}}},{"in_successful_contract_call":false,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"read_data_byte"}],"data":{"u64":"3028"}}}}},{"in_successful_contract_call":false,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"write_data_byte"}],"data":{"u64":"1276"}}}}},{"in_successful_contract_call":false,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"read_code_byte"}],"data":{"u64":"50032"}}}}},{"in_successful_contract_call":false,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"write_code_byte"}],"data":{"u64":"0"}}}}},{"in_successful_contract_call":false,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"emit_event"}],"data":{"u64":"2"}}}}},{"in_successful_contract_call":false,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"emit_event_byte"}],"data":{"u64":"460"}}}}},{"in_successful_contract_call":false,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"cpu_insn"}],"data":{"u64":"8808582"}}}}},{"in_successful_contract_call":false,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"mem_byte"}],"data":{"u64":"3010311"}}}}},{"in_successful_contract_call":false,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"invoke_time_nsecs"}],"data":{"u64":"1166673"}}}}},{"in_successful_contract_call":false,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"max_rw_key_byte"}],"data":{"u64":"168"}}}}},{"in_successful_contract_call":false,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"max_rw_data_byte"}],"data":{"u64":"508"}}}}},{"in_successful_contract_call":false,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"max_rw_code_byte"}],"data":{"u64":"50032"}}}}},{"in_successful_contract_call":false,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"max_emit_event_byte"}],"data":{"u64":"244"}}}}}]}}},"events":{"contract_events":[],"diagnostic_events":[{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"fn_call"},{"bytes":"eb0aa9d8d625796902fa9be6341291de077e8dd523a7378e46a4a6152da8183b"},{"symbol":"submit"}],"data":{"vec":[{"address":"GBQUFZ3QRIP6VQ74BV6KJGBEJ7YFE4WGRCB4YCMGTXFEMYLXNI2CC2AK"},{"address":"GBQUFZ3QRIP6VQ74BV6KJGBEJ7YFE4WGRCB4YCMGTXFEMYLXNI2CC2AK"},{"address":"GBQUFZ3QRIP6VQ74BV6KJGBEJ7YFE4WGRCB4YCMGTXFEMYLXNI2CC2AK"},{"vec":[{"map":[{"key":{"symbol":"address"},"val":{"address":"CCW67TSZV3SSS2HXMBQ5JFGCKJNXKZM7UQUWUZPUTHXSTZLEO7SJMI75"}},{"key":{"symbol":"amount"},"val":{"i128":"10000990"}},{"key":{"symbol":"request_type"},"val":{"u32":3}}]}]}]}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":"CDVQVKOY2YSXS2IC7KN6MNASSHPAO7UN2UR2ON4OI2SKMFJNVAMDX6DP","type_":"contract","body":{"v0":{"topics":[{"symbol":"withdraw_collateral"},{"address":"CCW67TSZV3SSS2HXMBQ5JFGCKJNXKZM7UQUWUZPUTHXSTZLEO7SJMI75"},{"address":"GBQUFZ3QRIP6VQ74BV6KJGBEJ7YFE4WGRCB4YCMGTXFEMYLXNI2CC2AK"}],"data":{"vec":[{"i128":"10000990"},{"i128":"8933481"}]}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":"CDVQVKOY2YSXS2IC7KN6MNASSHPAO7UN2UR2ON4OI2SKMFJNVAMDX6DP","type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"fn_call"},{"bytes":"adefce59aee52968f76061d494c2525b75659fa4296a65f499ef29e56477e496"},{"symbol":"transfer"}],"data":{"vec":[{"address":"CDVQVKOY2YSXS2IC7KN6MNASSHPAO7UN2UR2ON4OI2SKMFJNVAMDX6DP"},{"address":"GBQUFZ3QRIP6VQ74BV6KJGBEJ7YFE4WGRCB4YCMGTXFEMYLXNI2CC2AK"},{"i128":"10000990"}]}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":"CCW67TSZV3SSS2HXMBQ5JFGCKJNXKZM7UQUWUZPUTHXSTZLEO7SJMI75","type_":"contract","body":{"v0":{"topics":[{"symbol":"transfer"},{"address":"CDVQVKOY2YSXS2IC7KN6MNASSHPAO7UN2UR2ON4OI2SKMFJNVAMDX6DP"},{"address":"GBQUFZ3QRIP6VQ74BV6KJGBEJ7YFE4WGRCB4YCMGTXFEMYLXNI2CC2AK"},{"string":"USDC:GA5ZSEJYB37JRC5AVCIA5MOP4RHTM335X2KGX3IHOJAPP5RE34K4KZVN"}],"data":{"i128":"10000990"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":"CCW67TSZV3SSS2HXMBQ5JFGCKJNXKZM7UQUWUZPUTHXSTZLEO7SJMI75","type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"fn_return"},{"symbol":"transfer"}],"data":"void"}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":"CDVQVKOY2YSXS2IC7KN6MNASSHPAO7UN2UR2ON4OI2SKMFJNVAMDX6DP","type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"fn_return"},{"symbol":"submit"}],"data":{"map":[{"key":{"symbol":"collateral"},"val":{"map":[]}},{"key":{"symbol":"liabilities"},"val":{"map":[]}},{"key":{"symbol":"supply"},"val":{"map":[]}}]}}}}},{"in_successful_contract_call":false,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"read_entry"}],"data":{"u64":"11"}}}}},{"in_successful_contract_call":false,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"write_entry"}],"data":{"u64":"5"}}}}},{"in_successful_contract_call":false,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"ledger_read_byte"}],"data":{"u64":"53060"}}}}},{"in_successful_contract_call":false,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"ledger_write_byte"}],"data":{"u64":"1276"}}}}},{"in_successful_contract_call":false,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"read_key_byte"}],"data":{"u64":"1008"}}}}},{"in_successful_contract_call":false,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"write_key_byte"}],"data":{"u64":"0"}}}}},{"in_successful_contract_call":false,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"read_data_byte"}],"data":{"u64":"3028"}}}}},{"in_successful_contract_call":false,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"write_data_byte"}],"data":{"u64":"1276"}}}}},{"in_successful_contract_call":false,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"read_code_byte"}],"data":{"u64":"50032"}}}}},{"in_successful_contract_call":false,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"write_code_byte"}],"data":{"u64":"0"}}}}},{"in_successful_contract_call":false,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"emit_event"}],"data":{"u64":"2"}}}}},{"in_successful_contract_call":false,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"emit_event_byte"}],"data":{"u64":"460"}}}}},{"in_successful_contract_call":false,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"cpu_insn"}],"data":{"u64":"8808582"}}}}},{"in_successful_contract_call":false,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"mem_byte"}],"data":{"u64":"3010311"}}}}},{"in_successful_contract_call":false,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"invoke_time_nsecs"}],"data":{"u64":"1166673"}}}}},{"in_successful_contract_call":false,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"max_rw_key_byte"}],"data":{"u64":"168"}}}}},{"in_successful_contract_call":false,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"max_rw_data_byte"}],"data":{"u64":"508"}}}}},{"in_successful_contract_call":false,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"max_rw_code_byte"}],"data":{"u64":"50032"}}}}},{"in_successful_contract_call":false,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"max_emit_event_byte"}],"data":{"u64":"244"}}}}}],"transaction_events":[]}}"#;
}
