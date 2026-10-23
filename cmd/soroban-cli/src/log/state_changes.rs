use soroban_rpc::LedgerEntryChange;
use tracing::debug;

use crate::print::Print;
use crate::utils::XDR_DEPTH_LIMIT;
use crate::xdr::{self, LedgerEntry, LedgerKey, Limits, ReadXdr};

#[derive(serde::Serialize, Debug, Clone, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DecodedStateChange {
    Created {
        key: serde_json::Value,
        after: serde_json::Value,
    },
    Deleted {
        key: serde_json::Value,
        before: serde_json::Value,
    },
    Updated {
        key: serde_json::Value,
        before: serde_json::Value,
        after: serde_json::Value,
    },
}

pub fn format_key_summary(key: &LedgerKey) -> String {
    match key {
        LedgerKey::Account(acc) => {
            format!("Account ({})", acc.account_id)
        }
        LedgerKey::Trustline(t) => {
            let asset_str = match &t.asset {
                xdr::TrustLineAsset::Native => "native".to_string(),
                xdr::TrustLineAsset::CreditAlphanum4(a) => {
                    let code = std::str::from_utf8(a.asset_code.0.as_slice()).unwrap_or("???");
                    format!("{code}:{}", a.issuer)
                }
                xdr::TrustLineAsset::CreditAlphanum12(a) => {
                    let code = std::str::from_utf8(a.asset_code.0.as_slice()).unwrap_or("???");
                    format!("{code}:{}", a.issuer)
                }
                xdr::TrustLineAsset::PoolShare(p) => {
                    format!("pool:{}", hex::encode(p.0 .0))
                }
            };
            format!("Trustline ({}, {asset_str})", t.account_id)
        }
        LedgerKey::ContractData(cd) => {
            let key_repr =
                soroban_spec_tools::to_string(&cd.key).unwrap_or_else(|_| format!("{:?}", cd.key));
            format!(
                "ContractData ({}, {:?}, {key_repr})",
                cd.contract, cd.durability
            )
        }
        LedgerKey::ContractCode(cc) => {
            format!("ContractCode ({})", hex::encode(cc.hash.0))
        }
        LedgerKey::ConfigSetting(cs) => {
            format!("ConfigSetting ({:?})", cs.config_setting_id)
        }
        LedgerKey::Ttl(ttl) => {
            format!("TTL ({})", hex::encode(ttl.key_hash.0))
        }
        LedgerKey::Offer(o) => {
            format!("Offer ({}, id: {})", o.seller_id, o.offer_id)
        }
        LedgerKey::ClaimableBalance(cb) => {
            format!("ClaimableBalance ({:?})", cb.balance_id)
        }
        LedgerKey::LiquidityPool(lp) => {
            format!("LiquidityPool ({})", hex::encode(lp.liquidity_pool_id.0 .0))
        }
        other => format!("{other:?}"),
    }
}

pub fn format_entry_summary(entry: &LedgerEntry) -> String {
    match &entry.data {
        xdr::LedgerEntryData::Account(acc) => {
            format!(
                "Account (balance: {} stroops, seq: {})",
                acc.balance, acc.seq_num.0
            )
        }
        xdr::LedgerEntryData::Trustline(t) => {
            format!("Trustline (balance: {}, limit: {})", t.balance, t.limit)
        }
        xdr::LedgerEntryData::ContractData(cd) => {
            let val_repr =
                soroban_spec_tools::to_string(&cd.val).unwrap_or_else(|_| format!("{:?}", cd.val));
            format!("ContractData (val: {val_repr})")
        }
        xdr::LedgerEntryData::ContractCode(cc) => {
            format!("ContractCode (size: {} bytes)", cc.code.len())
        }
        xdr::LedgerEntryData::Ttl(ttl) => {
            format!("TTL (live_until_ledger_seq: {})", ttl.live_until_ledger_seq)
        }
        xdr::LedgerEntryData::Offer(o) => {
            format!("Offer (amount: {}, price: {:?})", o.amount, o.price)
        }
        other => format!("{other:?}"),
    }
}

pub fn decode_state_change(change: &LedgerEntryChange) -> Option<DecodedStateChange> {
    match change {
        LedgerEntryChange::Created { key, after } => {
            let k = LedgerKey::from_xdr_base64(key, Limits::depth(XDR_DEPTH_LIMIT)).ok()?;
            let a = LedgerEntry::from_xdr_base64(after, Limits::depth(XDR_DEPTH_LIMIT)).ok()?;
            Some(DecodedStateChange::Created {
                key: serde_json::to_value(&k).ok()?,
                after: serde_json::to_value(&a).ok()?,
            })
        }
        LedgerEntryChange::Deleted { key, before } => {
            let k = LedgerKey::from_xdr_base64(key, Limits::depth(XDR_DEPTH_LIMIT)).ok()?;
            let b = LedgerEntry::from_xdr_base64(before, Limits::depth(XDR_DEPTH_LIMIT)).ok()?;
            Some(DecodedStateChange::Deleted {
                key: serde_json::to_value(&k).ok()?,
                before: serde_json::to_value(&b).ok()?,
            })
        }
        LedgerEntryChange::Updated { key, before, after } => {
            let k = LedgerKey::from_xdr_base64(key, Limits::depth(XDR_DEPTH_LIMIT)).ok()?;
            let b = LedgerEntry::from_xdr_base64(before, Limits::depth(XDR_DEPTH_LIMIT)).ok()?;
            let a = LedgerEntry::from_xdr_base64(after, Limits::depth(XDR_DEPTH_LIMIT)).ok()?;
            Some(DecodedStateChange::Updated {
                key: serde_json::to_value(&k).ok()?,
                before: serde_json::to_value(&b).ok()?,
                after: serde_json::to_value(&a).ok()?,
            })
        }
    }
}

pub fn all(state_changes: &[LedgerEntryChange]) {
    for (index, change) in state_changes.iter().enumerate() {
        if let Some(decoded) = decode_state_change(change) {
            let json = serde_json::to_string(&decoded).unwrap_or_default();
            debug!("state_change {index}: {json}");
        } else {
            let raw_json = serde_json::to_string(change).unwrap_or_default();
            debug!("state_change {index} (raw): {raw_json}");
        }
    }
}

pub fn state_changes(changes: &[LedgerEntryChange], print: &Print) {
    if changes.is_empty() {
        return;
    }

    for change in changes {
        match change {
            LedgerEntryChange::Created { key, after } => {
                let decoded_key =
                    LedgerKey::from_xdr_base64(key, Limits::depth(XDR_DEPTH_LIMIT)).ok();
                let decoded_after =
                    LedgerEntry::from_xdr_base64(after, Limits::depth(XDR_DEPTH_LIMIT)).ok();

                let key_summary = decoded_key
                    .as_ref()
                    .map(format_key_summary)
                    .unwrap_or_else(|| key.clone());

                let after_summary = decoded_after
                    .as_ref()
                    .map(format_entry_summary)
                    .unwrap_or_else(|| after.clone());

                print.bucketln(format!("State Change: Created - {key_summary}"));
                print.println(format!("  After: {after_summary}"));
            }
            LedgerEntryChange::Deleted { key, before } => {
                let decoded_key =
                    LedgerKey::from_xdr_base64(key, Limits::depth(XDR_DEPTH_LIMIT)).ok();
                let decoded_before =
                    LedgerEntry::from_xdr_base64(before, Limits::depth(XDR_DEPTH_LIMIT)).ok();

                let key_summary = decoded_key
                    .as_ref()
                    .map(format_key_summary)
                    .unwrap_or_else(|| key.clone());

                let before_summary = decoded_before
                    .as_ref()
                    .map(format_entry_summary)
                    .unwrap_or_else(|| before.clone());

                print.bucketln(format!("State Change: Deleted - {key_summary}"));
                print.println(format!("  Before: {before_summary}"));
            }
            LedgerEntryChange::Updated { key, before, after } => {
                let decoded_key =
                    LedgerKey::from_xdr_base64(key, Limits::depth(XDR_DEPTH_LIMIT)).ok();
                let decoded_before =
                    LedgerEntry::from_xdr_base64(before, Limits::depth(XDR_DEPTH_LIMIT)).ok();
                let decoded_after =
                    LedgerEntry::from_xdr_base64(after, Limits::depth(XDR_DEPTH_LIMIT)).ok();

                let key_summary = decoded_key
                    .as_ref()
                    .map(format_key_summary)
                    .unwrap_or_else(|| key.clone());

                let before_summary = decoded_before
                    .as_ref()
                    .map(format_entry_summary)
                    .unwrap_or_else(|| before.clone());

                let after_summary = decoded_after
                    .as_ref()
                    .map(format_entry_summary)
                    .unwrap_or_else(|| after.clone());

                print.bucketln(format!("State Change: Updated - {key_summary}"));
                print.println(format!("  Before: {before_summary}"));
                print.println(format!("  After:  {after_summary}"));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xdr::{
        AccountEntry, AccountId, ContractDataDurability, ContractDataEntry, LedgerEntryData,
        LedgerEntryExt, LedgerKeyAccount, LedgerKeyContractData, PublicKey, ScAddress, ScVal,
        SequenceNumber, Uint256, WriteXdr,
    };

    #[test]
    fn test_format_and_decode_state_changes() {
        let account_id = AccountId(PublicKey::PublicKeyTypeEd25519(Uint256([7; 32])));
        let key = LedgerKey::Account(LedgerKeyAccount {
            account_id: account_id.clone(),
        });
        let key_base64 = key.to_xdr_base64(Limits::depth(XDR_DEPTH_LIMIT)).unwrap();

        let before_entry = LedgerEntry {
            last_modified_ledger_seq: 100,
            data: LedgerEntryData::Account(AccountEntry {
                account_id: account_id.clone(),
                balance: 10_000_000,
                seq_num: SequenceNumber(1),
                num_sub_entries: 0,
                inflation_dest: None,
                flags: 0,
                home_domain: Default::default(),
                thresholds: Default::default(),
                signers: Default::default(),
                ext: Default::default(),
            }),
            ext: LedgerEntryExt::V0,
        };
        let before_base64 = before_entry
            .to_xdr_base64(Limits::depth(XDR_DEPTH_LIMIT))
            .unwrap();

        let after_entry = LedgerEntry {
            last_modified_ledger_seq: 101,
            data: LedgerEntryData::Account(AccountEntry {
                account_id,
                balance: 20_000_000,
                seq_num: SequenceNumber(2),
                num_sub_entries: 0,
                inflation_dest: None,
                flags: 0,
                home_domain: Default::default(),
                thresholds: Default::default(),
                signers: Default::default(),
                ext: Default::default(),
            }),
            ext: LedgerEntryExt::V0,
        };
        let after_base64 = after_entry
            .to_xdr_base64(Limits::depth(XDR_DEPTH_LIMIT))
            .unwrap();

        let change = LedgerEntryChange::Updated {
            key: key_base64,
            before: before_base64,
            after: after_base64,
        };

        let decoded = decode_state_change(&change);
        assert!(decoded.is_some());

        // Ensure state_changes and all do not panic
        let print = Print::new(true);
        state_changes(&[change.clone()], &print);
        all(&[change]);
    }

    #[test]
    fn test_contract_data_state_change() {
        let contract = ScAddress::Contract(stellar_xdr::ContractId(xdr::Hash([9; 32])));
        let key = LedgerKey::ContractData(LedgerKeyContractData {
            contract: contract.clone(),
            key: ScVal::Symbol("my_key".try_into().unwrap()),
            durability: ContractDataDurability::Persistent,
        });
        let key_base64 = key.to_xdr_base64(Limits::depth(XDR_DEPTH_LIMIT)).unwrap();

        let after_entry = LedgerEntry {
            last_modified_ledger_seq: 50,
            data: LedgerEntryData::ContractData(ContractDataEntry {
                contract,
                key: ScVal::Symbol("my_key".try_into().unwrap()),
                durability: ContractDataDurability::Persistent,
                val: ScVal::U32(999),
                ext: Default::default(),
            }),
            ext: LedgerEntryExt::V0,
        };
        let after_base64 = after_entry
            .to_xdr_base64(Limits::depth(XDR_DEPTH_LIMIT))
            .unwrap();

        let change = LedgerEntryChange::Created {
            key: key_base64,
            after: after_base64,
        };

        let decoded = decode_state_change(&change);
        assert!(decoded.is_some());

        let print = Print::new(true);
        state_changes(&[change], &print);
    }
}
