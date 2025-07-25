use super::args;
use crate::{
    commands::global,
    config::network,
    rpc,
    xdr::{self, Hash, SorobanTransactionMetaExt, TransactionEnvelope, TransactionMeta},
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

const DEFAULT_FEE_VALUE: i64 = 0;
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
    pub fee_charged: i64,
    pub resource_fee_charged: i64,
    pub inclusion_fee_charged: i64,
    pub non_refundable_resource_fee_charged: i64,
    pub refundable_resource_fee_charged: i64,
    pub max_fee: i64,
    pub max_resource_fee: i64,
}

impl FeeTable {
    fn new_from_transaction_response(resp: &GetTransactionResponse) -> Result<Self, Error> {
        let tx_result = resp.result.clone().ok_or(Error::None {
            field: "tx_result".to_string(),
        })?; // fee charged
        let tx_meta = resp.result_meta.clone().ok_or(Error::None {
            field: "tx_meta".to_string(),
        })?; // resource fees
        let tx_envelope = resp.envelope.clone().ok_or(Error::None {
            field: "tx_envelope".to_string(),
        })?; // max fees

        let fee_charged = tx_result.fee_charged;
        let (non_refundable_resource_fee_charged, refundable_resource_fee_charged) =
            Self::resource_fees_charged(&tx_meta);

        let (max_fee, max_resource_fee) = Self::max_fees(&tx_envelope);

        let resource_fee_charged =
            non_refundable_resource_fee_charged + refundable_resource_fee_charged;
        let inclusion_fee_charged = fee_charged - resource_fee_charged;
        Ok(FeeTable {
            fee_charged,
            resource_fee_charged,
            inclusion_fee_charged,
            non_refundable_resource_fee_charged,
            refundable_resource_fee_charged,
            max_fee,
            max_resource_fee,
        })
    }

    fn max_fees(tx_envelope: &TransactionEnvelope) -> (i64, i64) {
        match tx_envelope {
            TransactionEnvelope::TxV0(transaction_v0_envelope) => {
                let fee = transaction_v0_envelope.tx.fee;
                (fee.into(), DEFAULT_FEE_VALUE)
            }
            TransactionEnvelope::Tx(transaction_v1_envelope) => {
                let tx = transaction_v1_envelope.tx.clone();
                let fee = tx.fee;
                let resource_fee = match tx.ext {
                    xdr::TransactionExt::V0 => DEFAULT_FEE_VALUE,
                    xdr::TransactionExt::V1(soroban_transaction_data) => {
                        soroban_transaction_data.resource_fee
                    }
                };

                (fee.into(), resource_fee)
            }
            TransactionEnvelope::TxFeeBump(fee_bump_transaction_envelope) => {
                let fee = fee_bump_transaction_envelope.tx.fee;
                (fee, DEFAULT_FEE_VALUE)
            }
        }
    }

    fn resource_fees_charged(tx_meta: &TransactionMeta) -> (i64, i64) {
        let (non_refundable_resource_fee_charged, refundable_resource_fee_charged) =
            match tx_meta.clone() {
                TransactionMeta::V0(_) | TransactionMeta::V1(_) | TransactionMeta::V2(_) => {
                    (DEFAULT_FEE_VALUE, DEFAULT_FEE_VALUE)
                }
                TransactionMeta::V3(meta) => {
                    if let Some(soroban_meta) = meta.soroban_meta {
                        match soroban_meta.ext {
                            SorobanTransactionMetaExt::V0 => (DEFAULT_FEE_VALUE, DEFAULT_FEE_VALUE),
                            SorobanTransactionMetaExt::V1(v1) => (
                                v1.total_non_refundable_resource_fee_charged,
                                v1.total_refundable_resource_fee_charged,
                            ),
                        }
                    } else {
                        (DEFAULT_FEE_VALUE, DEFAULT_FEE_VALUE)
                    }
                }
                TransactionMeta::V4(meta) => {
                    if let Some(soroban_meta) = meta.soroban_meta {
                        match soroban_meta.ext {
                            SorobanTransactionMetaExt::V0 => (DEFAULT_FEE_VALUE, DEFAULT_FEE_VALUE),
                            SorobanTransactionMetaExt::V1(v1) => (
                                v1.total_non_refundable_resource_fee_charged,
                                v1.total_refundable_resource_fee_charged,
                            ),
                        }
                    } else {
                        (DEFAULT_FEE_VALUE, DEFAULT_FEE_VALUE)
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

    fn proposed_inclusion_fee(&self) -> i64 {
        self.max_fee - self.max_resource_fee
    }

    fn refunded(&self) -> i64 {
        self.max_fee - self.fee_charged
    }

    fn refundable_fee_proposed(&self) -> i64 {
        self.max_resource_fee - self.non_refundable_resource_fee_charged
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
                INCLUSION_FEE_TITLE,
                self.proposed_inclusion_fee()
            )),
            Cell::new(&format!("{RESOURCE_FEE_TITLE}: {}", self.max_resource_fee)).with_hspan(3),
        ]));

        table.add_row(Row::new(vec![
            Cell::new(&format!(
                "{}: {}",
                INCLUSION_FEE_TITLE,
                self.proposed_inclusion_fee()
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

        let expected_fee_charged = 60_537;
        let expected_non_refundable_charged = 60_358;
        let expected_refundable_charged = 79;
        let expected_resource_fee_charged =
            expected_non_refundable_charged + expected_refundable_charged;
        let expected_inclusion_fee_charged = expected_fee_charged - expected_resource_fee_charged;
        let expected_max_fee = 105_447;
        let expected_max_resource_fee = 105_347;

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
        assert_eq!(fee_table.max_fee, expected_max_fee);
        assert_eq!(fee_table.max_resource_fee, expected_max_resource_fee);
    }

    #[test]
    fn classic_tx_fee_table() {
        let resp = classic_tx_response().unwrap();
        let fee_table = FeeTable::new_from_transaction_response(&resp).unwrap();

        let expected_fee_charged = 100;
        let expected_non_refundable_charged = DEFAULT_FEE_VALUE;
        let expected_refundable_charged = DEFAULT_FEE_VALUE;
        let expected_resource_fee_charged =
            expected_non_refundable_charged + expected_refundable_charged;
        let expected_inclusion_fee_charged = expected_fee_charged - expected_resource_fee_charged;
        let expected_max_fee = 100;
        let expected_max_resource_fee = DEFAULT_FEE_VALUE;

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
        assert_eq!(fee_table.max_fee, expected_max_fee);
        assert_eq!(fee_table.max_resource_fee, expected_max_resource_fee);
    }

    #[test]
    fn fee_bump_tx_fee_table() {
        let resp = fee_bump_tx_response().unwrap();
        let fee_table = FeeTable::new_from_transaction_response(&resp).unwrap();

        let expected_fee_charged = 200;
        let expected_non_refundable_charged = DEFAULT_FEE_VALUE;
        let expected_refundable_charged = DEFAULT_FEE_VALUE;
        let expected_resource_fee_charged =
            expected_non_refundable_charged + expected_refundable_charged;
        let expected_inclusion_fee_charged = expected_fee_charged - expected_resource_fee_charged;
        let expected_max_fee = 400;
        let expected_max_resource_fee = DEFAULT_FEE_VALUE;

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
        assert_eq!(fee_table.max_fee, expected_max_fee);
        assert_eq!(fee_table.max_resource_fee, expected_max_resource_fee);
    }

    fn soroban_tx_response() -> Result<GetTransactionResponse, serde_json::Error> {
        serde_json::from_str(SOROBAN_TX_RESPONSE)
    }

    fn classic_tx_response() -> Result<GetTransactionResponse, serde_json::Error> {
        serde_json::from_str(CLASSIC_TX_RESPONSE)
    }

    fn fee_bump_tx_response() -> Result<GetTransactionResponse, serde_json::Error> {
        serde_json::from_str(FEE_BUMP_TX_RESPONSE)
    }

    const SOROBAN_TX_RESPONSE: &str = r#"{"status":"SUCCESS","envelope":{"tx":{"tx":{"source_account":"GDWREJ5HETNIDTQKXJZPA6LRSJMFUCO4T2DFEJYSZ2XVWRTMUG64AL4B","fee":105447,"seq_num":"2062499130114054","cond":"none","memo":"none","operations":[{"source_account":null,"body":{"invoke_host_function":{"host_function":{"invoke_contract":{"contract_address":"CCFNZO33IO6GDTPLWWRJ5F34UBXEBOSYGSQJJGVLAJNNULU26CRZR6TM","function_name":"inc","args":[]}},"auth":[]}}}],"ext":{"v1":{"ext":"v0","resources":{"footprint":{"read_only":[{"contract_data":{"contract":"CCFNZO33IO6GDTPLWWRJ5F34UBXEBOSYGSQJJGVLAJNNULU26CRZR6TM","key":"ledger_key_contract_instance","durability":"persistent"}},{"contract_code":{"hash":"2a41e16cb574fc372ee81f02c1d775365d9d39002cc630bd162a4dcaeb153161"}}],"read_write":[{"contract_data":{"contract":"CCFNZO33IO6GDTPLWWRJ5F34UBXEBOSYGSQJJGVLAJNNULU26CRZR6TM","key":{"symbol":"COUNTER"},"durability":"persistent"}}]},"instructions":2099865,"disk_read_bytes":8008,"write_bytes":80},"resource_fee":"105347"}}},"signatures":[{"hint":"6ca1bdc0","signature":"b8120310a5db9b5fd295f00d9fd7ebc26e34d14b27a00a5827aa89a18027fb7d69fb1b11e3b6408885602627b228256fde049aee2045fea7d088327302e4ea04"}]}},"result":{"fee_charged":"60537","result":{"tx_success":[{"op_inner":{"invoke_host_function":{"success":"e18456c437deb4d21dceee8db938ac8bcea25405af8df02d9225104e5d53e185"}}}]},"ext":"v0"},"result_meta":{"v3":{"ext":"v0","tx_changes_before":[{"state":{"last_modified_ledger_seq":480745,"data":{"account":{"account_id":"GDWREJ5HETNIDTQKXJZPA6LRSJMFUCO4T2DFEJYSZ2XVWRTMUG64AL4B","balance":"99907097552","seq_num":"2062499130114053","num_sub_entries":1,"inflation_dest":null,"flags":0,"home_domain":"","thresholds":"01000000","signers":[],"ext":{"v1":{"liabilities":{"buying":"0","selling":"0"},"ext":{"v2":{"num_sponsored":0,"num_sponsoring":0,"signer_sponsoring_i_ds":[],"ext":{"v3":{"ext":"v0","seq_ledger":480418,"seq_time":"1752671669"}}}}}}}},"ext":"v0"}},{"updated":{"last_modified_ledger_seq":480745,"data":{"account":{"account_id":"GDWREJ5HETNIDTQKXJZPA6LRSJMFUCO4T2DFEJYSZ2XVWRTMUG64AL4B","balance":"99907097552","seq_num":"2062499130114054","num_sub_entries":1,"inflation_dest":null,"flags":0,"home_domain":"","thresholds":"01000000","signers":[],"ext":{"v1":{"liabilities":{"buying":"0","selling":"0"},"ext":{"v2":{"num_sponsored":0,"num_sponsoring":0,"signer_sponsoring_i_ds":[],"ext":{"v3":{"ext":"v0","seq_ledger":480745,"seq_time":"1752673305"}}}}}}}},"ext":"v0"}}],"operations":[{"changes":[{"state":{"last_modified_ledger_seq":480290,"data":{"contract_data":{"ext":"v0","contract":"CCFNZO33IO6GDTPLWWRJ5F34UBXEBOSYGSQJJGVLAJNNULU26CRZR6TM","key":{"symbol":"COUNTER"},"durability":"persistent","val":{"u32":2}}},"ext":"v0"}},{"updated":{"last_modified_ledger_seq":480745,"data":{"contract_data":{"ext":"v0","contract":"CCFNZO33IO6GDTPLWWRJ5F34UBXEBOSYGSQJJGVLAJNNULU26CRZR6TM","key":{"symbol":"COUNTER"},"durability":"persistent","val":{"u32":3}}},"ext":"v0"}}]}],"tx_changes_after":[{"state":{"last_modified_ledger_seq":480745,"data":{"account":{"account_id":"GDWREJ5HETNIDTQKXJZPA6LRSJMFUCO4T2DFEJYSZ2XVWRTMUG64AL4B","balance":"99907097552","seq_num":"2062499130114054","num_sub_entries":1,"inflation_dest":null,"flags":0,"home_domain":"","thresholds":"01000000","signers":[],"ext":{"v1":{"liabilities":{"buying":"0","selling":"0"},"ext":{"v2":{"num_sponsored":0,"num_sponsoring":0,"signer_sponsoring_i_ds":[],"ext":{"v3":{"ext":"v0","seq_ledger":480745,"seq_time":"1752673305"}}}}}}}},"ext":"v0"}},{"updated":{"last_modified_ledger_seq":480745,"data":{"account":{"account_id":"GDWREJ5HETNIDTQKXJZPA6LRSJMFUCO4T2DFEJYSZ2XVWRTMUG64AL4B","balance":"99907142462","seq_num":"2062499130114054","num_sub_entries":1,"inflation_dest":null,"flags":0,"home_domain":"","thresholds":"01000000","signers":[],"ext":{"v1":{"liabilities":{"buying":"0","selling":"0"},"ext":{"v2":{"num_sponsored":0,"num_sponsoring":0,"signer_sponsoring_i_ds":[],"ext":{"v3":{"ext":"v0","seq_ledger":480745,"seq_time":"1752673305"}}}}}}}},"ext":"v0"}}],"soroban_meta":{"ext":{"v1":{"ext":"v0","total_non_refundable_resource_fee_charged":"60358","total_refundable_resource_fee_charged":"79","rent_fee_charged":"0"}},"events":[],"return_value":{"u32":3},"diagnostic_events":[{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"fn_call"},{"bytes":"8adcbb7b43bc61cdebb5a29e977ca06e40ba5834a0949aab025ada2e9af0a398"},{"symbol":"inc"}],"data":"void"}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":"CCFNZO33IO6GDTPLWWRJ5F34UBXEBOSYGSQJJGVLAJNNULU26CRZR6TM","type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"log"}],"data":{"vec":[{"string":"count: {}"},{"u32":2}]}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":"CCFNZO33IO6GDTPLWWRJ5F34UBXEBOSYGSQJJGVLAJNNULU26CRZR6TM","type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"fn_return"},{"symbol":"inc"}],"data":{"u32":3}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"read_entry"}],"data":{"u64":"6"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"write_entry"}],"data":{"u64":"1"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"ledger_read_byte"}],"data":{"u64":"8008"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"ledger_write_byte"}],"data":{"u64":"80"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"read_key_byte"}],"data":{"u64":"144"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"write_key_byte"}],"data":{"u64":"0"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"read_data_byte"}],"data":{"u64":"184"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"write_data_byte"}],"data":{"u64":"80"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"read_code_byte"}],"data":{"u64":"7824"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"write_code_byte"}],"data":{"u64":"0"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"emit_event"}],"data":{"u64":"0"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"emit_event_byte"}],"data":{"u64":"8"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"cpu_insn"}],"data":{"u64":"2006689"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"mem_byte"}],"data":{"u64":"1492062"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"invoke_time_nsecs"}],"data":{"u64":"561706"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"max_rw_key_byte"}],"data":{"u64":"60"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"max_rw_data_byte"}],"data":{"u64":"104"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"max_rw_code_byte"}],"data":{"u64":"7824"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"max_emit_event_byte"}],"data":{"u64":"0"}}}}}]}}},"events":{"contract_events":[],"diagnostic_events":[{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"fn_call"},{"bytes":"8adcbb7b43bc61cdebb5a29e977ca06e40ba5834a0949aab025ada2e9af0a398"},{"symbol":"inc"}],"data":"void"}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":"CCFNZO33IO6GDTPLWWRJ5F34UBXEBOSYGSQJJGVLAJNNULU26CRZR6TM","type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"log"}],"data":{"vec":[{"string":"count: {}"},{"u32":2}]}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":"CCFNZO33IO6GDTPLWWRJ5F34UBXEBOSYGSQJJGVLAJNNULU26CRZR6TM","type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"fn_return"},{"symbol":"inc"}],"data":{"u32":3}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"read_entry"}],"data":{"u64":"6"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"write_entry"}],"data":{"u64":"1"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"ledger_read_byte"}],"data":{"u64":"8008"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"ledger_write_byte"}],"data":{"u64":"80"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"read_key_byte"}],"data":{"u64":"144"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"write_key_byte"}],"data":{"u64":"0"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"read_data_byte"}],"data":{"u64":"184"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"write_data_byte"}],"data":{"u64":"80"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"read_code_byte"}],"data":{"u64":"7824"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"write_code_byte"}],"data":{"u64":"0"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"emit_event"}],"data":{"u64":"0"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"emit_event_byte"}],"data":{"u64":"8"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"cpu_insn"}],"data":{"u64":"2006689"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"mem_byte"}],"data":{"u64":"1492062"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"invoke_time_nsecs"}],"data":{"u64":"561706"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"max_rw_key_byte"}],"data":{"u64":"60"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"max_rw_data_byte"}],"data":{"u64":"104"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"max_rw_code_byte"}],"data":{"u64":"7824"}}}}},{"in_successful_contract_call":true,"event":{"ext":"v0","contract_id":null,"type_":"diagnostic","body":{"v0":{"topics":[{"symbol":"core_metrics"},{"symbol":"max_emit_event_byte"}],"data":{"u64":"0"}}}}}],"transaction_events":[]}}"#;

    const CLASSIC_TX_RESPONSE: &str = r#"{"status":"SUCCESS","envelope":{"tx":{"tx":{"source_account":"GDWREJ5HETNIDTQKXJZPA6LRSJMFUCO4T2DFEJYSZ2XVWRTMUG64AL4B","fee":100,"seq_num":"2062499130114053","cond":"none","memo":"none","operations":[{"source_account":null,"body":{"manage_data":{"data_name":"test","data_value":"abcdef"}}}],"ext":"v0"},"signatures":[{"hint":"6ca1bdc0","signature":"a12761eee624d0a15f731b6e63201c55978d714a28d167e80441092afb11a06549056199e589ff511d376299782cde796169a1781b7ecad93cbe68ac3a768d05"}]}},"result":{"fee_charged":"100","result":{"tx_success":[{"op_inner":{"manage_data":"success"}}]},"ext":"v0"},"result_meta":{"v3":{"ext":"v0","tx_changes_before":[{"state":{"last_modified_ledger_seq":480418,"data":{"account":{"account_id":"GDWREJ5HETNIDTQKXJZPA6LRSJMFUCO4T2DFEJYSZ2XVWRTMUG64AL4B","balance":"99907202999","seq_num":"2062499130114052","num_sub_entries":0,"inflation_dest":null,"flags":0,"home_domain":"","thresholds":"01000000","signers":[],"ext":{"v1":{"liabilities":{"buying":"0","selling":"0"},"ext":{"v2":{"num_sponsored":0,"num_sponsoring":0,"signer_sponsoring_i_ds":[],"ext":{"v3":{"ext":"v0","seq_ledger":480290,"seq_time":"1752671028"}}}}}}}},"ext":"v0"}},{"updated":{"last_modified_ledger_seq":480418,"data":{"account":{"account_id":"GDWREJ5HETNIDTQKXJZPA6LRSJMFUCO4T2DFEJYSZ2XVWRTMUG64AL4B","balance":"99907202999","seq_num":"2062499130114053","num_sub_entries":0,"inflation_dest":null,"flags":0,"home_domain":"","thresholds":"01000000","signers":[],"ext":{"v1":{"liabilities":{"buying":"0","selling":"0"},"ext":{"v2":{"num_sponsored":0,"num_sponsoring":0,"signer_sponsoring_i_ds":[],"ext":{"v3":{"ext":"v0","seq_ledger":480418,"seq_time":"1752671669"}}}}}}}},"ext":"v0"}}],"operations":[{"changes":[{"created":{"last_modified_ledger_seq":480418,"data":{"data":{"account_id":"GDWREJ5HETNIDTQKXJZPA6LRSJMFUCO4T2DFEJYSZ2XVWRTMUG64AL4B","data_name":"test","data_value":"abcdef","ext":"v0"}},"ext":"v0"}},{"state":{"last_modified_ledger_seq":480418,"data":{"account":{"account_id":"GDWREJ5HETNIDTQKXJZPA6LRSJMFUCO4T2DFEJYSZ2XVWRTMUG64AL4B","balance":"99907202999","seq_num":"2062499130114053","num_sub_entries":0,"inflation_dest":null,"flags":0,"home_domain":"","thresholds":"01000000","signers":[],"ext":{"v1":{"liabilities":{"buying":"0","selling":"0"},"ext":{"v2":{"num_sponsored":0,"num_sponsoring":0,"signer_sponsoring_i_ds":[],"ext":{"v3":{"ext":"v0","seq_ledger":480418,"seq_time":"1752671669"}}}}}}}},"ext":"v0"}},{"updated":{"last_modified_ledger_seq":480418,"data":{"account":{"account_id":"GDWREJ5HETNIDTQKXJZPA6LRSJMFUCO4T2DFEJYSZ2XVWRTMUG64AL4B","balance":"99907202999","seq_num":"2062499130114053","num_sub_entries":1,"inflation_dest":null,"flags":0,"home_domain":"","thresholds":"01000000","signers":[],"ext":{"v1":{"liabilities":{"buying":"0","selling":"0"},"ext":{"v2":{"num_sponsored":0,"num_sponsoring":0,"signer_sponsoring_i_ds":[],"ext":{"v3":{"ext":"v0","seq_ledger":480418,"seq_time":"1752671669"}}}}}}}},"ext":"v0"}}]}],"tx_changes_after":[],"soroban_meta":null}},"events":{"contract_events":[],"diagnostic_events":[],"transaction_events":[]}}"#;

    const FEE_BUMP_TX_RESPONSE: &str = r#"{"status":"SUCCESS","envelope":{"tx_fee_bump":{"tx":{"fee_source":"GD5EJEGJM5PWKZ4WBJFMTHHY3VNUDJDU55N24ODPIPNYKBRRJCCIA44P","fee":"400","inner_tx":{"tx":{"tx":{"source_account":"GDWREJ5HETNIDTQKXJZPA6LRSJMFUCO4T2DFEJYSZ2XVWRTMUG64AL4B","fee":100,"seq_num":"2062499130114055","cond":{"time":{"min_time":"0","max_time":"1752675654"}},"memo":"none","operations":[{"source_account":null,"body":{"payment":{"destination":"GDBFMEGF2EVTNISNTYVOOYGXAEP5A353YJCPDRGUH3L6GMIDATR4BWY6","asset":"native","amount":"100000000"}}}],"ext":"v0"},"signatures":[{"hint":"6ca1bdc0","signature":"ce5f19bac1e1a57f6f54a7d4f5729fd2db8755dac425fd61220111e3d4436dfd52e9f0f0098ea0c07fc3e65c69f19f7d1f440adc7fa9937662bd9268fb7cb00c"}]}},"ext":"v0"},"signatures":[{"hint":"31488480","signature":"789b8261c481532c7f8933ed1b32d9fb270d9acc044774dda1986f20aba8248592975f25eec1aabe374978fcc10a19b9797c834d686465a4d225b01d0c57020e"}]}},"result":{"fee_charged":"200","result":{"tx_fee_bump_inner_success":{"transaction_hash":"b6b9591c8c00d1aa9212ef0345e6b1ccd56f9a362e463a1f6237423d09dbcab8","result":{"fee_charged":"100","result":{"tx_success":[{"op_inner":{"payment":"success"}}]},"ext":"v0"}}},"ext":"v0"},"result_meta":{"v3":{"ext":"v0","tx_changes_before":[{"state":{"last_modified_ledger_seq":481209,"data":{"account":{"account_id":"GD5EJEGJM5PWKZ4WBJFMTHHY3VNUDJDU55N24ODPIPNYKBRRJCCIA44P","balance":"99999999800","seq_num":"2065887859310592","num_sub_entries":0,"inflation_dest":null,"flags":0,"home_domain":"","thresholds":"01000000","signers":[],"ext":"v0"}},"ext":"v0"}},{"updated":{"last_modified_ledger_seq":481209,"data":{"account":{"account_id":"GD5EJEGJM5PWKZ4WBJFMTHHY3VNUDJDU55N24ODPIPNYKBRRJCCIA44P","balance":"99999999800","seq_num":"2065887859310592","num_sub_entries":0,"inflation_dest":null,"flags":0,"home_domain":"","thresholds":"01000000","signers":[],"ext":"v0"}},"ext":"v0"}},{"state":{"last_modified_ledger_seq":481027,"data":{"account":{"account_id":"GDWREJ5HETNIDTQKXJZPA6LRSJMFUCO4T2DFEJYSZ2XVWRTMUG64AL4B","balance":"199907142462","seq_num":"2062499130114054","num_sub_entries":1,"inflation_dest":null,"flags":0,"home_domain":"","thresholds":"01000000","signers":[],"ext":{"v1":{"liabilities":{"buying":"0","selling":"0"},"ext":{"v2":{"num_sponsored":0,"num_sponsoring":0,"signer_sponsoring_i_ds":[],"ext":{"v3":{"ext":"v0","seq_ledger":480745,"seq_time":"1752673305"}}}}}}}},"ext":"v0"}},{"updated":{"last_modified_ledger_seq":481209,"data":{"account":{"account_id":"GDWREJ5HETNIDTQKXJZPA6LRSJMFUCO4T2DFEJYSZ2XVWRTMUG64AL4B","balance":"199907142462","seq_num":"2062499130114055","num_sub_entries":1,"inflation_dest":null,"flags":0,"home_domain":"","thresholds":"01000000","signers":[],"ext":{"v1":{"liabilities":{"buying":"0","selling":"0"},"ext":{"v2":{"num_sponsored":0,"num_sponsoring":0,"signer_sponsoring_i_ds":[],"ext":{"v3":{"ext":"v0","seq_ledger":481209,"seq_time":"1752675627"}}}}}}}},"ext":"v0"}}],"operations":[{"changes":[{"state":{"last_modified_ledger_seq":481209,"data":{"account":{"account_id":"GDWREJ5HETNIDTQKXJZPA6LRSJMFUCO4T2DFEJYSZ2XVWRTMUG64AL4B","balance":"199907142462","seq_num":"2062499130114055","num_sub_entries":1,"inflation_dest":null,"flags":0,"home_domain":"","thresholds":"01000000","signers":[],"ext":{"v1":{"liabilities":{"buying":"0","selling":"0"},"ext":{"v2":{"num_sponsored":0,"num_sponsoring":0,"signer_sponsoring_i_ds":[],"ext":{"v3":{"ext":"v0","seq_ledger":481209,"seq_time":"1752675627"}}}}}}}},"ext":"v0"}},{"updated":{"last_modified_ledger_seq":481209,"data":{"account":{"account_id":"GDWREJ5HETNIDTQKXJZPA6LRSJMFUCO4T2DFEJYSZ2XVWRTMUG64AL4B","balance":"199807142462","seq_num":"2062499130114055","num_sub_entries":1,"inflation_dest":null,"flags":0,"home_domain":"","thresholds":"01000000","signers":[],"ext":{"v1":{"liabilities":{"buying":"0","selling":"0"},"ext":{"v2":{"num_sponsored":0,"num_sponsoring":0,"signer_sponsoring_i_ds":[],"ext":{"v3":{"ext":"v0","seq_ledger":481209,"seq_time":"1752675627"}}}}}}}},"ext":"v0"}},{"state":{"last_modified_ledger_seq":481014,"data":{"account":{"account_id":"GDBFMEGF2EVTNISNTYVOOYGXAEP5A353YJCPDRGUH3L6GMIDATR4BWY6","balance":"100000000000","seq_num":"2065939398918144","num_sub_entries":0,"inflation_dest":null,"flags":0,"home_domain":"","thresholds":"01000000","signers":[],"ext":"v0"}},"ext":"v0"}},{"updated":{"last_modified_ledger_seq":481209,"data":{"account":{"account_id":"GDBFMEGF2EVTNISNTYVOOYGXAEP5A353YJCPDRGUH3L6GMIDATR4BWY6","balance":"100100000000","seq_num":"2065939398918144","num_sub_entries":0,"inflation_dest":null,"flags":0,"home_domain":"","thresholds":"01000000","signers":[],"ext":"v0"}},"ext":"v0"}}]}],"tx_changes_after":[],"soroban_meta":null}},"events":{"contract_events":[],"diagnostic_events":[],"transaction_events":[]}}"#;
}
