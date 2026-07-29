use std::{fmt::Debug, path::Path, str::FromStr};

use crate::{
    assembled::simulate_and_assemble_transaction,
    log::extract_events,
    tx::sim_sign_and_send_tx,
    xdr::{
        ConfigSettingEntry, ConfigSettingId, Error as XdrError, ExtensionPoint, LedgerEntry,
        LedgerEntryChange, LedgerEntryData, LedgerFootprint, LedgerKey, LedgerKeyConfigSetting,
        Limits, Memo, Operation, OperationBody, Preconditions, RestoreFootprintOp, SequenceNumber,
        SorobanResources, SorobanTransactionData, SorobanTransactionDataExt, Transaction,
        TransactionEnvelope, TransactionExt, TransactionMeta, TransactionMetaV3, TransactionMetaV4,
        TransactionV1Envelope, TtlEntry, VecM, WriteXdr,
    },
};
use clap::Parser;
use stellar_strkey::DecodeError;

use crate::commands::tx::fetch;
use crate::{
    commands::{
        contract::extend,
        global,
        txn_result::{TxnEnvelopeResult, TxnResult},
        HEADING_TRANSACTION,
    },
    config::{self, data, locator, network},
    key, rpc, wasm, Pwd,
};

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub key: key::Args,

    /// Number of ledgers to extend the entry
    #[arg(long)]
    pub ledgers_to_extend: Option<u32>,

    /// Only print the new Time To Live ledger
    #[arg(long)]
    pub ttl_ledger_only: bool,

    #[command(flatten)]
    pub config: config::Args,

    #[command(flatten)]
    pub resources: crate::resources::Args,

    /// Build the transaction and only write the base64 xdr to stdout
    #[arg(long, help_heading = HEADING_TRANSACTION)]
    pub build_only: bool,

    /// Simulate the restore instead of submitting it, and report whether it
    /// fits within the network's per-transaction resource limits
    #[arg(long, conflicts_with = "build_only", help_heading = HEADING_TRANSACTION)]
    pub dry_run: bool,
}

impl FromStr for Cmd {
    type Err = clap::error::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use clap::{CommandFactory, FromArgMatches};
        Self::from_arg_matches_mut(&mut Self::command().get_matches_from(s.split_whitespace()))
    }
}

impl Pwd for Cmd {
    fn set_pwd(&mut self, pwd: &Path) {
        self.config.set_pwd(pwd);
    }
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("parsing key {key}: {error}")]
    CannotParseKey {
        key: String,
        error: soroban_spec_tools::Error,
    },

    #[error("parsing XDR key {key}: {error}")]
    CannotParseXdrKey { key: String, error: XdrError },

    #[error("cannot parse contract ID {0}: {1}")]
    CannotParseContractId(String, DecodeError),

    #[error(transparent)]
    Config(#[from] config::Error),

    #[error("either `--key` or `--key-xdr` are required")]
    KeyIsRequired,

    #[error("xdr processing error: {0}")]
    Xdr(#[from] XdrError),

    #[error("Ledger entry not found")]
    LedgerEntryNotFound,

    #[error(transparent)]
    Locator(#[from] locator::Error),

    #[error("missing operation result")]
    MissingOperationResult,

    #[error(transparent)]
    Rpc(#[from] rpc::Error),

    #[error(transparent)]
    Wasm(#[from] wasm::Error),

    #[error(transparent)]
    Key(#[from] key::Error),

    #[error(transparent)]
    Extend(#[from] extend::Error),

    #[error(transparent)]
    Data(#[from] data::Error),

    #[error(transparent)]
    Network(#[from] network::Error),

    #[error(transparent)]
    Fee(#[from] fetch::fee::Error),

    #[error(transparent)]
    Fetch(#[from] fetch::Error),

    #[error("config setting {0} not found in network config")]
    MissingConfigSetting(&'static str),
}

impl Cmd {
    #[allow(clippy::too_many_lines)]
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        if self.dry_run {
            return self.run_dry_run(&self.config, global_args.quiet).await;
        }

        let res = self
            .execute(&self.config, global_args.quiet, global_args.no_cache)
            .await?
            .to_envelope();
        let expiration_ledger_seq = match res {
            TxnEnvelopeResult::TxnEnvelope(tx) => {
                println!("{}", tx.to_xdr_base64(Limits::none())?);
                return Ok(());
            }
            TxnEnvelopeResult::Res(res) => res,
        };
        if let Some(ledgers_to_extend) = self.ledgers_to_extend {
            extend::Cmd {
                key: self.key.clone(),
                ledgers_to_extend,
                config: self.config.clone(),
                resources: self.resources.clone(),
                ttl_ledger_only: false,
                build_only: self.build_only,
            }
            .run(global_args)
            .await?;
        } else {
            println!("New ttl ledger: {expiration_ledger_seq}");
        }

        Ok(())
    }

    pub async fn execute(
        &self,
        config: &config::Args,
        quiet: bool,
        no_cache: bool,
    ) -> Result<TxnResult<u32>, Error> {
        let print = crate::print::Print::new(quiet);
        let network = config.get_network()?;
        tracing::trace!(?network);
        let entry_keys = self.key.parse_keys(&config.locator, &network)?;
        let client = network.rpc_client()?;
        client
            .verify_network_passphrase(Some(&network.network_passphrase))
            .await?;

        let tx = self.build_tx(config, &client, &entry_keys).await?;
        if self.build_only {
            return Ok(TxnResult::Txn(tx));
        }

        let res = sim_sign_and_send_tx::<Error>(
            &client,
            &tx,
            config,
            &self.resources,
            &[],
            // Footprint restore is not an InvokeHostFunction op, so the RPC does
            // not accept an auth mode.
            None,
            quiet,
            no_cache,
        )
        .await?;

        let meta = res
            .result_meta
            .as_ref()
            .ok_or(Error::MissingOperationResult)?;

        tracing::trace!(?meta);

        let events = extract_events(meta);

        crate::log::event::all(&events);
        crate::log::event::contract(&events, &print);

        // The transaction from core will succeed regardless of whether it actually found &
        // restored the entry, so we have to inspect the result meta to tell if it worked or not.
        let changes = match meta {
            TransactionMeta::V4(TransactionMetaV4 { operations, .. }) => {
                // Simply check if there is exactly one entry here. We only support restoring a single
                // entry via this command (which we should fix separately, but).
                if operations.is_empty() {
                    return Err(Error::LedgerEntryNotFound);
                }

                operations[0].changes.clone()
            }
            TransactionMeta::V3(TransactionMetaV3 { operations, .. }) => {
                // Simply check if there is exactly one entry here. We only support restoring a single
                // entry via this command (which we should fix separately, but).
                if operations.is_empty() {
                    return Err(Error::LedgerEntryNotFound);
                }

                operations[0].changes.clone()
            }
            _ => return Err(Error::LedgerEntryNotFound),
        };
        tracing::debug!("Changes:\nlen:{}\n{changes:#?}", changes.len());

        if changes.is_empty() {
            print.infoln("No changes detected, transaction was a no-op.");
            let entry = client.get_full_ledger_entries(&entry_keys).await?;
            // The fetch after a no-op can return no entries (e.g. the entry
            // was evicted in the meantime), so avoid indexing into an empty
            // vec (which would panic).
            let extension = entry
                .entries
                .first()
                .ok_or(Error::LedgerEntryNotFound)?
                .live_until_ledger_seq
                .unwrap_or_default();

            return Ok(TxnResult::Res(extension));
        }

        Ok(TxnResult::Res(
            parse_changes(&changes.to_vec()).ok_or(Error::LedgerEntryNotFound)?,
        ))
    }

    /// Builds the unsigned `RestoreFootprint` transaction with the given
    /// footprint entries in its read-write set.
    async fn build_tx(
        &self,
        config: &config::Args,
        client: &rpc::Client,
        entry_keys: &[LedgerKey],
    ) -> Result<Box<Transaction>, Error> {
        let source_account = config.source_account()?;

        // Get the account sequence number
        let account_details = client
            .get_account(&source_account.clone().to_string())
            .await?;
        let sequence: i64 = account_details.seq_num.into();

        Ok(Box::new(Transaction {
            source_account,
            fee: config.get_inclusion_fee()?,
            seq_num: SequenceNumber(sequence + 1),
            cond: Preconditions::None,
            memo: Memo::None,
            operations: vec![Operation {
                source_account: None,
                body: OperationBody::RestoreFootprint(RestoreFootprintOp {
                    ext: ExtensionPoint::V0,
                }),
            }]
            .try_into()?,
            ext: TransactionExt::V1(SorobanTransactionData {
                ext: SorobanTransactionDataExt::V0,
                resources: SorobanResources {
                    footprint: LedgerFootprint {
                        read_only: vec![].try_into()?,
                        read_write: entry_keys.to_vec().try_into()?,
                    },
                    instructions: self.resources.instructions.unwrap_or_default(),
                    disk_read_bytes: 0,
                    write_bytes: 0,
                },
                resource_fee: 0,
            }),
        }))
    }

    /// Simulates the restore (instead of submitting it) and reports whether the
    /// resulting transaction fits within the network's per-transaction resource
    /// limits, listing every limit and by how much any is exceeded.
    async fn run_dry_run(&self, config: &config::Args, quiet: bool) -> Result<(), Error> {
        let print = crate::print::Print::new(quiet);
        let network = config.get_network()?;
        let entry_keys = self.key.parse_keys(&config.locator, &network)?;
        let client = network.rpc_client()?;
        client
            .verify_network_passphrase(Some(&network.network_passphrase))
            .await?;

        let tx = self.build_tx(config, &client, &entry_keys).await?;

        print.infoln("Simulating restore transaction…");
        let assembled = simulate_and_assemble_transaction(
            &client,
            &tx,
            self.resources.resource_config(),
            self.resources.resource_fee,
            // Footprint restore is not an InvokeHostFunction op, so the RPC does
            // not accept an auth mode.
            None,
        )
        .await?;

        let usage = ResourceUsage::from_transaction(assembled.transaction())?;
        let limits = fetch_resource_limits(&client).await?;
        let checks = check_limits(&usage, &limits);

        print.infoln("Per-transaction resource limits (from network config):");
        for check in &checks {
            let status = if check.exceeded() {
                format!("EXCEEDS by {}", check.overage())
            } else {
                "ok".to_string()
            };
            print.blankln(format!(
                "{}: {} / {} ({status})",
                check.name, check.used, check.limit
            ));
        }
        print.blankln(
            "(transaction size excludes signatures, so it is a lower-bound estimate; \
             the signed transaction will be slightly larger)",
        );

        if fits(&checks) {
            print.checkln("This restore fits within a single transaction.");
        } else {
            print.errorln(
                "This restore does NOT fit within a single transaction. Restoring fewer keys \
                 per transaction would be required. (Computing the minimum number of \
                 transactions and grouping keys is not yet implemented.)",
            );
        }

        Ok(())
    }
}

/// The network's per-transaction resource limits, fetched from config settings.
#[derive(Debug, Clone, Copy)]
struct ResourceLimits {
    instructions: i64,
    disk_read_entries: u32,
    disk_read_bytes: u32,
    write_ledger_entries: u32,
    write_bytes: u32,
    tx_size_bytes: u32,
}

/// The resource usage of a simulated transaction.
#[derive(Debug, Clone, Copy)]
struct ResourceUsage {
    instructions: u32,
    disk_read_entries: u32,
    disk_read_bytes: u32,
    write_ledger_entries: u32,
    write_bytes: u32,
    tx_size_bytes: u32,
}

impl ResourceUsage {
    /// Extracts the resource usage from an assembled (post-simulation) transaction.
    fn from_transaction(tx: &Transaction) -> Result<Self, Error> {
        let TransactionExt::V1(SorobanTransactionData {
            resources:
                SorobanResources {
                    footprint,
                    instructions,
                    disk_read_bytes,
                    write_bytes,
                },
            ..
        }) = &tx.ext
        else {
            return Err(Error::MissingOperationResult);
        };

        // The transaction size the network limits is the size of the whole
        // envelope. We don't have signatures yet at simulation time, so this is
        // a close lower-bound estimate.
        let tx_env = TransactionEnvelope::Tx(TransactionV1Envelope {
            tx: tx.clone(),
            signatures: VecM::default(),
        });
        let tx_size_bytes = u32::try_from(tx_env.to_xdr(Limits::none())?.len()).unwrap_or(u32::MAX);

        Ok(Self {
            instructions: *instructions,
            // Every footprint entry (read-only + read-write) is read from disk.
            disk_read_entries: u32::try_from(
                footprint.read_only.len() + footprint.read_write.len(),
            )
            .unwrap_or(u32::MAX),
            disk_read_bytes: *disk_read_bytes,
            write_ledger_entries: u32::try_from(footprint.read_write.len()).unwrap_or(u32::MAX),
            write_bytes: *write_bytes,
            tx_size_bytes,
        })
    }
}

/// A single resource limit comparison.
struct LimitCheck {
    name: &'static str,
    used: u64,
    limit: u64,
}

impl LimitCheck {
    fn exceeded(&self) -> bool {
        self.used > self.limit
    }

    fn overage(&self) -> u64 {
        self.used.saturating_sub(self.limit)
    }
}

/// Compares the simulated resource usage against every per-transaction limit.
fn check_limits(usage: &ResourceUsage, limits: &ResourceLimits) -> Vec<LimitCheck> {
    vec![
        LimitCheck {
            name: "instructions",
            used: u64::from(usage.instructions),
            limit: u64::try_from(limits.instructions).unwrap_or(0),
        },
        LimitCheck {
            name: "disk read entries",
            used: u64::from(usage.disk_read_entries),
            limit: u64::from(limits.disk_read_entries),
        },
        LimitCheck {
            name: "disk read bytes",
            used: u64::from(usage.disk_read_bytes),
            limit: u64::from(limits.disk_read_bytes),
        },
        LimitCheck {
            name: "write ledger entries",
            used: u64::from(usage.write_ledger_entries),
            limit: u64::from(limits.write_ledger_entries),
        },
        LimitCheck {
            name: "write bytes",
            used: u64::from(usage.write_bytes),
            limit: u64::from(limits.write_bytes),
        },
        LimitCheck {
            name: "transaction size (bytes)",
            used: u64::from(usage.tx_size_bytes),
            limit: u64::from(limits.tx_size_bytes),
        },
    ]
}

/// Whether the transaction fits within all per-transaction limits.
fn fits(checks: &[LimitCheck]) -> bool {
    checks.iter().all(|check| !check.exceeded())
}

/// Fetches the per-transaction resource limits from the network's config
/// settings via RPC (the same mechanism as `stellar network settings`).
async fn fetch_resource_limits(client: &rpc::Client) -> Result<ResourceLimits, Error> {
    let keys = [
        ConfigSettingId::ContractComputeV0,
        ConfigSettingId::ContractLedgerCostV0,
        ConfigSettingId::ContractBandwidthV0,
    ]
    .into_iter()
    .map(|config_setting_id| LedgerKey::ConfigSetting(LedgerKeyConfigSetting { config_setting_id }))
    .collect::<Vec<_>>();

    let mut compute = None;
    let mut ledger_cost = None;
    let mut bandwidth = None;
    for entry in client.get_full_ledger_entries(&keys).await?.entries {
        match entry.val {
            LedgerEntryData::ConfigSetting(ConfigSettingEntry::ContractComputeV0(c)) => {
                compute = Some(c);
            }
            LedgerEntryData::ConfigSetting(ConfigSettingEntry::ContractLedgerCostV0(c)) => {
                ledger_cost = Some(c);
            }
            LedgerEntryData::ConfigSetting(ConfigSettingEntry::ContractBandwidthV0(c)) => {
                bandwidth = Some(c);
            }
            _ => {}
        }
    }

    let compute = compute.ok_or(Error::MissingConfigSetting("ContractComputeV0"))?;
    let ledger_cost = ledger_cost.ok_or(Error::MissingConfigSetting("ContractLedgerCostV0"))?;
    let bandwidth = bandwidth.ok_or(Error::MissingConfigSetting("ContractBandwidthV0"))?;

    Ok(ResourceLimits {
        instructions: compute.tx_max_instructions,
        disk_read_entries: ledger_cost.tx_max_disk_read_entries,
        disk_read_bytes: ledger_cost.tx_max_disk_read_bytes,
        write_ledger_entries: ledger_cost.tx_max_write_ledger_entries,
        write_bytes: ledger_cost.tx_max_write_bytes,
        tx_size_bytes: bandwidth.tx_max_size_bytes,
    })
}

fn parse_changes(changes: &[LedgerEntryChange]) -> Option<u32> {
    changes
        .iter()
        .filter_map(|change| match change {
            LedgerEntryChange::Restored(LedgerEntry {
                data:
                    LedgerEntryData::Ttl(TtlEntry {
                        live_until_ledger_seq,
                        ..
                    }),
                ..
            })
            | LedgerEntryChange::Updated(LedgerEntry {
                data:
                    LedgerEntryData::Ttl(TtlEntry {
                        live_until_ledger_seq,
                        ..
                    }),
                ..
            })
            | LedgerEntryChange::Created(LedgerEntry {
                data:
                    LedgerEntryData::Ttl(TtlEntry {
                        live_until_ledger_seq,
                        ..
                    }),
                ..
            }) => Some(*live_until_ledger_seq),
            _ => None,
        })
        .max()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xdr::{
        ContractDataDurability::Persistent, ContractDataEntry, ContractId, Hash, LedgerEntry,
        LedgerEntryChange, LedgerEntryData, ScAddress, ScSymbol, ScVal, SequenceNumber, StringM,
        TtlEntry,
    };

    #[test]
    fn test_parse_changes_two_changes_restored() {
        // Test the original expected format with 2 changes
        let ttl_entry = TtlEntry {
            live_until_ledger_seq: 12345,
            key_hash: Hash([0; 32]),
        };

        let changes = vec![
            LedgerEntryChange::State(LedgerEntry {
                data: LedgerEntryData::Ttl(ttl_entry.clone()),
                last_modified_ledger_seq: 0,
                ext: crate::xdr::LedgerEntryExt::V0,
            }),
            LedgerEntryChange::Restored(LedgerEntry {
                data: LedgerEntryData::Ttl(ttl_entry),
                last_modified_ledger_seq: 0,
                ext: crate::xdr::LedgerEntryExt::V0,
            }),
        ];

        let result = parse_changes(&changes);
        assert_eq!(result, Some(12345));
    }

    #[test]
    fn test_parse_two_changes_that_had_expired() {
        let ttl_entry = TtlEntry {
            live_until_ledger_seq: 55555,
            key_hash: Hash([0; 32]),
        };

        let counter = "COUNTER".parse::<StringM<32>>().unwrap();
        let contract_data_entry = ContractDataEntry {
            ext: ExtensionPoint::default(),
            contract: ScAddress::Contract(ContractId(Hash([0; 32]))),
            key: ScVal::Symbol(ScSymbol(counter)),
            durability: Persistent,
            val: ScVal::U32(1),
        };

        let changes = vec![
            LedgerEntryChange::Restored(LedgerEntry {
                data: LedgerEntryData::Ttl(ttl_entry.clone()),
                last_modified_ledger_seq: 37429,
                ext: crate::xdr::LedgerEntryExt::V0,
            }),
            LedgerEntryChange::Restored(LedgerEntry {
                data: LedgerEntryData::ContractData(contract_data_entry.clone()),
                last_modified_ledger_seq: 37429,
                ext: crate::xdr::LedgerEntryExt::V0,
            }),
        ];

        let result = parse_changes(&changes);
        assert_eq!(result, Some(55555));
    }

    #[test]
    fn test_parse_changes_two_changes_updated() {
        // Test the original expected format with 2 changes, but second change is Updated
        let ttl_entry = TtlEntry {
            live_until_ledger_seq: 67890,
            key_hash: Hash([0; 32]),
        };

        let changes = vec![
            LedgerEntryChange::State(LedgerEntry {
                data: LedgerEntryData::Ttl(ttl_entry.clone()),
                last_modified_ledger_seq: 0,
                ext: crate::xdr::LedgerEntryExt::V0,
            }),
            LedgerEntryChange::Updated(LedgerEntry {
                data: LedgerEntryData::Ttl(ttl_entry),
                last_modified_ledger_seq: 0,
                ext: crate::xdr::LedgerEntryExt::V0,
            }),
        ];

        let result = parse_changes(&changes);
        assert_eq!(result, Some(67890));
    }

    #[test]
    fn test_parse_changes_two_changes_created() {
        // Test the original expected format with 2 changes, but second change is Created
        let ttl_entry = TtlEntry {
            live_until_ledger_seq: 11111,
            key_hash: Hash([0; 32]),
        };

        let changes = vec![
            LedgerEntryChange::State(LedgerEntry {
                data: LedgerEntryData::Ttl(ttl_entry.clone()),
                last_modified_ledger_seq: 0,
                ext: crate::xdr::LedgerEntryExt::V0,
            }),
            LedgerEntryChange::Created(LedgerEntry {
                data: LedgerEntryData::Ttl(ttl_entry),
                last_modified_ledger_seq: 0,
                ext: crate::xdr::LedgerEntryExt::V0,
            }),
        ];

        let result = parse_changes(&changes);
        assert_eq!(result, Some(11111));
    }

    #[test]
    fn test_parse_changes_single_change_restored() {
        // Test the new single change format with Restored type
        let ttl_entry = TtlEntry {
            live_until_ledger_seq: 22222,
            key_hash: Hash([0; 32]),
        };

        let changes = vec![LedgerEntryChange::Restored(LedgerEntry {
            data: LedgerEntryData::Ttl(ttl_entry),
            last_modified_ledger_seq: 0,
            ext: crate::xdr::LedgerEntryExt::V0,
        })];

        let result = parse_changes(&changes);
        assert_eq!(result, Some(22222));
    }

    #[test]
    fn test_parse_changes_single_change_updated() {
        // Test the new single change format with Updated type
        let ttl_entry = TtlEntry {
            live_until_ledger_seq: 33333,
            key_hash: Hash([0; 32]),
        };

        let changes = vec![LedgerEntryChange::Updated(LedgerEntry {
            data: LedgerEntryData::Ttl(ttl_entry),
            last_modified_ledger_seq: 0,
            ext: crate::xdr::LedgerEntryExt::V0,
        })];

        let result = parse_changes(&changes);
        assert_eq!(result, Some(33333));
    }

    #[test]
    fn test_parse_changes_single_change_created() {
        // Test the new single change format with Created type
        let ttl_entry = TtlEntry {
            live_until_ledger_seq: 44444,
            key_hash: Hash([0; 32]),
        };

        let changes = vec![LedgerEntryChange::Created(LedgerEntry {
            data: LedgerEntryData::Ttl(ttl_entry),
            last_modified_ledger_seq: 0,
            ext: crate::xdr::LedgerEntryExt::V0,
        })];

        let result = parse_changes(&changes);
        assert_eq!(result, Some(44444));
    }

    #[test]
    fn test_parse_changes_invalid_two_changes() {
        // Test invalid 2-change format (not TTL data)
        let not_ttl_change = LedgerEntryChange::Restored(LedgerEntry {
            data: LedgerEntryData::Account(crate::xdr::AccountEntry {
                account_id: crate::xdr::AccountId(crate::xdr::PublicKey::PublicKeyTypeEd25519(
                    crate::xdr::Uint256([0; 32]),
                )),
                balance: 0,
                seq_num: SequenceNumber(0),
                num_sub_entries: 0,
                inflation_dest: None,
                flags: 0,
                home_domain: crate::xdr::String32::default(),
                thresholds: crate::xdr::Thresholds::default(),
                signers: crate::xdr::VecM::default(),
                ext: crate::xdr::AccountEntryExt::V0,
            }),
            last_modified_ledger_seq: 0,
            ext: crate::xdr::LedgerEntryExt::V0,
        });

        let changes = vec![not_ttl_change.clone(), not_ttl_change];
        let result = parse_changes(&changes);
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_changes_invalid_single_change() {
        // Test invalid single change format (not TTL data)
        let changes = vec![LedgerEntryChange::Restored(LedgerEntry {
            data: LedgerEntryData::Account(crate::xdr::AccountEntry {
                account_id: crate::xdr::AccountId(crate::xdr::PublicKey::PublicKeyTypeEd25519(
                    crate::xdr::Uint256([0; 32]),
                )),
                balance: 0,
                seq_num: SequenceNumber(0),
                num_sub_entries: 0,
                inflation_dest: None,
                flags: 0,
                home_domain: crate::xdr::String32::default(),
                thresholds: crate::xdr::Thresholds::default(),
                signers: crate::xdr::VecM::default(),
                ext: crate::xdr::AccountEntryExt::V0,
            }),
            last_modified_ledger_seq: 0,
            ext: crate::xdr::LedgerEntryExt::V0,
        })];

        let result = parse_changes(&changes);
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_changes_empty_changes() {
        // Test empty changes array
        let changes = vec![];

        let result = parse_changes(&changes);
        assert_eq!(result, None);
    }

    fn limits() -> ResourceLimits {
        ResourceLimits {
            instructions: 100_000_000,
            disk_read_entries: 40,
            disk_read_bytes: 200_000,
            write_ledger_entries: 20,
            write_bytes: 100_000,
            tx_size_bytes: 70_000,
        }
    }

    fn usage() -> ResourceUsage {
        ResourceUsage {
            instructions: 0,
            disk_read_entries: 5,
            disk_read_bytes: 1_000,
            write_ledger_entries: 5,
            write_bytes: 1_000,
            tx_size_bytes: 1_000,
        }
    }

    #[test]
    fn test_check_limits_within_bounds_fits() {
        let checks = check_limits(&usage(), &limits());
        assert!(fits(&checks));
        assert!(checks.iter().all(|c| !c.exceeded()));
        assert!(checks.iter().all(|c| c.overage() == 0));
        // All six limits are reported.
        assert_eq!(checks.len(), 6);
    }

    #[test]
    fn test_check_limits_reports_all_exceeded() {
        // Blow past every single limit at once.
        let usage = ResourceUsage {
            instructions: u32::MAX,
            disk_read_entries: 100,
            disk_read_bytes: 1_000_000,
            write_ledger_entries: 100,
            write_bytes: 1_000_000,
            tx_size_bytes: 1_000_000,
        };
        let checks = check_limits(&usage, &limits());
        assert!(!fits(&checks));
        // Every limit is flagged, not just the first one that trips.
        assert!(checks.iter().all(LimitCheck::exceeded));
        assert_eq!(checks.iter().filter(|c| c.exceeded()).count(), 6);
    }

    #[test]
    fn test_check_limits_partial_exceeded_and_overage() {
        // Only write_bytes and transaction size exceed.
        let usage = ResourceUsage {
            instructions: 0,
            disk_read_entries: 5,
            disk_read_bytes: 1_000,
            write_ledger_entries: 5,
            write_bytes: 150_000,
            tx_size_bytes: 80_000,
        };
        let checks = check_limits(&usage, &limits());
        assert!(!fits(&checks));

        let exceeded: Vec<_> = checks.iter().filter(|c| c.exceeded()).collect();
        assert_eq!(exceeded.len(), 2);

        let write_bytes = checks.iter().find(|c| c.name == "write bytes").unwrap();
        assert!(write_bytes.exceeded());
        assert_eq!(write_bytes.overage(), 50_000);

        let tx_size = checks
            .iter()
            .find(|c| c.name == "transaction size (bytes)")
            .unwrap();
        assert!(tx_size.exceeded());
        assert_eq!(tx_size.overage(), 10_000);

        // A limit that is not exceeded reports zero overage.
        let disk_read_bytes = checks.iter().find(|c| c.name == "disk read bytes").unwrap();
        assert!(!disk_read_bytes.exceeded());
        assert_eq!(disk_read_bytes.overage(), 0);
    }

    #[test]
    fn test_check_limits_at_exact_limit_fits() {
        // Usage exactly equal to the limit must count as fitting.
        let usage = ResourceUsage {
            instructions: 0,
            disk_read_entries: 40,
            disk_read_bytes: 200_000,
            write_ledger_entries: 20,
            write_bytes: 100_000,
            tx_size_bytes: 70_000,
        };
        let checks = check_limits(&usage, &limits());
        assert!(fits(&checks));
    }
}
