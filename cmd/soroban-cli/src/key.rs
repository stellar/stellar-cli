use crate::xdr::{
    self, AccountId, ContractId, Hash, LedgerKey, LedgerKeyContractCode, LedgerKeyContractData,
    Limits, MuxedEd25519Account, PublicKey, ReadXdr, ScAddress, ScMapEntry, ScString, ScSymbol,
    ScVal, Uint256, WriteXdr,
};
use crate::{
    commands::contract::Durability,
    config::{alias, locator, network::Network},
    utils::XDR_DEPTH_LIMIT,
    wasm,
};
use std::path::PathBuf;
use stellar_strkey::Strkey;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Spec(#[from] soroban_spec_tools::Error),
    #[error(transparent)]
    Xdr(#[from] xdr::Error),
    #[error("cannot parse contract ID {0}: {1}")]
    CannotParseContractId(String, stellar_strkey::DecodeError),
    #[error(transparent)]
    Wasm(#[from] wasm::Error),
    #[error(transparent)]
    Locator(#[from] locator::Error),
    #[error("cannot parse storage key: {0}")]
    CannotParseKey(String),
}

#[derive(Debug, clap::Args, Clone)]
#[group(skip)]
pub struct Args {
    /// Contract ID to which owns the data entries.
    /// If no keys provided the Contract's instance will be extended
    #[arg(
        long = "contract-id",
        visible_alias = "id",
        required_unless_present = "wasm",
        required_unless_present = "wasm_hash"
    )]
    pub contract_id: Option<alias::UnresolvedContract>,
    /// Storage key (symbol, address, number, or JSON)
    #[arg(long = "key", conflicts_with = "key_xdr")]
    pub key: Option<Vec<String>>,
    /// Storage key (base64-encoded XDR)
    #[arg(long = "key-xdr", conflicts_with = "key")]
    pub key_xdr: Option<Vec<String>>,
    /// Path to Wasm file of contract code to extend
    #[arg(
        long,
        conflicts_with = "contract_id",
        conflicts_with = "key",
        conflicts_with = "key_xdr",
        conflicts_with = "wasm_hash"
    )]
    pub wasm: Option<PathBuf>,
    /// Path to Wasm file of contract code to extend
    #[arg(
        long,
        conflicts_with = "contract_id",
        conflicts_with = "key",
        conflicts_with = "key_xdr",
        conflicts_with = "wasm"
    )]
    pub wasm_hash: Option<String>,
    /// Storage entry durability
    #[arg(long, value_enum, default_value = "persistent")]
    pub durability: Durability,
}

impl Args {
    pub fn parse_keys(
        &self,
        locator: &locator::Args,
        Network {
            network_passphrase, ..
        }: &Network,
    ) -> Result<Vec<LedgerKey>, Error> {
        let keys = if let Some(keys) = &self.key {
            keys.iter()
                .map(|key| parse_key_arg(key))
                .collect::<Result<Vec<_>, Error>>()?
        } else if let Some(keys) = &self.key_xdr {
            keys.iter()
                .map(|s| Ok(ScVal::from_xdr_base64(s, Limits::depth(XDR_DEPTH_LIMIT))?))
                .collect::<Result<Vec<_>, Error>>()?
        } else if let Some(wasm) = &self.wasm {
            return Ok(vec![crate::wasm::Args { wasm: wasm.clone() }.try_into()?]);
        } else if let Some(wasm_hash) = &self.wasm_hash {
            return Ok(vec![LedgerKey::ContractCode(LedgerKeyContractCode {
                hash: xdr::Hash(
                    soroban_spec_tools::utils::contract_id_from_str(wasm_hash)
                        .map_err(|e| Error::CannotParseContractId(wasm_hash.clone(), e))?,
                ),
            })]);
        } else {
            vec![ScVal::LedgerKeyContractInstance]
        };
        let contract = self
            .contract_id
            .as_ref()
            .unwrap()
            .resolve_contract_id(locator, network_passphrase)?;

        Ok(keys
            .into_iter()
            .map(|key| {
                LedgerKey::ContractData(LedgerKeyContractData {
                    contract: ScAddress::Contract(stellar_xdr::ContractId(xdr::Hash(contract.0))),
                    durability: (&self.durability).into(),
                    key,
                })
            })
            .collect())
    }
}

fn parse_address_scval(s: &str) -> Option<ScVal> {
    match Strkey::from_string(s).ok()? {
        Strkey::PublicKeyEd25519(p) => Some(ScVal::Address(ScAddress::Account(AccountId(
            PublicKey::PublicKeyTypeEd25519(Uint256(p.0)),
        )))),
        Strkey::Contract(c) => Some(ScVal::Address(ScAddress::Contract(ContractId(Hash(c.0))))),
        Strkey::MuxedAccountEd25519(m) => Some(ScVal::Address(ScAddress::MuxedAccount(
            MuxedEd25519Account {
                ed25519: Uint256(m.ed25519),
                id: m.id,
            },
        ))),
        _ => None,
    }
}

fn parse_str_scval(s: &str) -> Result<ScVal, Error> {
    if let Some(addr) = parse_address_scval(s) {
        return Ok(addr);
    }
    let bytes = s.as_bytes();
    if let Ok(sym) = xdr::StringM::<32>::try_from(bytes) {
        Ok(ScVal::Symbol(ScSymbol(sym)))
    } else if let Ok(str_val) = xdr::StringM::<{ u32::MAX }>::try_from(bytes) {
        Ok(ScVal::String(ScString(str_val)))
    } else {
        Err(Error::CannotParseKey(s.to_string()))
    }
}

fn json_to_scval(v: &serde_json::Value) -> Result<ScVal, Error> {
    match v {
        serde_json::Value::Null => Ok(ScVal::Void),
        serde_json::Value::Bool(b) => Ok(ScVal::Bool(*b)),
        serde_json::Value::Number(n) => {
            if let Some(u) = n.as_u64() {
                if let Ok(u32_val) = u32::try_from(u) {
                    Ok(ScVal::U32(u32_val))
                } else {
                    Ok(ScVal::U64(u))
                }
            } else if let Some(i) = n.as_i64() {
                if let Ok(i32_val) = i32::try_from(i) {
                    Ok(ScVal::I32(i32_val))
                } else {
                    Ok(ScVal::I64(i))
                }
            } else {
                Err(Error::CannotParseKey(v.to_string()))
            }
        }
        serde_json::Value::String(s) => parse_str_scval(s),
        serde_json::Value::Array(arr) => {
            let sc_vec: Vec<ScVal> = arr
                .iter()
                .map(json_to_scval)
                .collect::<Result<Vec<_>, Error>>()?;
            let vec_xdr = sc_vec.try_into()?;
            Ok(ScVal::Vec(Some(vec_xdr)))
        }
        serde_json::Value::Object(map) => {
            let mut entries = Vec::new();
            for (k, val) in map {
                let k_val = parse_str_scval(k)?;
                let v_val = json_to_scval(val)?;
                entries.push(ScMapEntry {
                    key: k_val,
                    val: v_val,
                });
            }
            entries.sort_by(|a, b| {
                let a_bytes = a.key.to_xdr(Limits::none()).unwrap_or_default();
                let b_bytes = b.key.to_xdr(Limits::none()).unwrap_or_default();
                a_bytes.cmp(&b_bytes)
            });
            let map_xdr = entries.try_into()?;
            Ok(ScVal::Map(Some(map_xdr)))
        }
    }
}

pub fn parse_key_arg(key_str: &str) -> Result<ScVal, Error> {
    // 1. If it's formatted as valid XDR-JSON representation of ScVal (e.g. {"symbol": "COUNTER"} or {"u32": 123})
    if let Ok(scval) = serde_json::from_str::<ScVal>(key_str) {
        return Ok(scval);
    }

    // 2. If it's a valid Stellar StrKey address (Account G..., Contract C..., or Muxed M...)
    if let Some(sc_address) = parse_address_scval(key_str) {
        return Ok(sc_address);
    }

    // 3. If it's a JSON string representing a structure, array, or object
    if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(key_str) {
        if !matches!(json_val, serde_json::Value::String(_)) {
            return json_to_scval(&json_val);
        }
    }

    // 4. Boolean literal
    if key_str == "true" {
        return Ok(ScVal::Bool(true));
    }
    if key_str == "false" {
        return Ok(ScVal::Bool(false));
    }

    // 5. Integer literal
    if let Ok(val) = key_str.parse::<u32>() {
        return Ok(ScVal::U32(val));
    }
    if let Ok(val) = key_str.parse::<u64>() {
        return Ok(ScVal::U64(val));
    }
    if let Ok(val) = key_str.parse::<i64>() {
        return Ok(ScVal::I64(val));
    }

    // 6. Default fallback: treat as Symbol (backward-compatible with original behavior)
    soroban_spec_tools::from_string_primitive(key_str, &xdr::ScSpecTypeDef::Symbol)
        .map_err(Error::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_key_arg_symbol() {
        let scval = parse_key_arg("COUNTER").unwrap();
        assert_eq!(
            scval,
            ScVal::Symbol(ScSymbol("COUNTER".try_into().unwrap()))
        );
    }

    #[test]
    fn test_parse_key_arg_account_address() {
        let addr_str = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF";
        let scval = parse_key_arg(addr_str).unwrap();
        match scval {
            ScVal::Address(ScAddress::Account(AccountId(PublicKey::PublicKeyTypeEd25519(
                Uint256(bytes),
            )))) => {
                assert_eq!(bytes, [0; 32]);
            }
            other => panic!("expected Account address, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_key_arg_contract_address() {
        let addr_str = "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4";
        let scval = parse_key_arg(addr_str).unwrap();
        match scval {
            ScVal::Address(ScAddress::Contract(ContractId(Hash(bytes)))) => {
                assert_eq!(bytes, [0; 32]);
            }
            other => panic!("expected Contract address, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_key_arg_number() {
        let scval = parse_key_arg("42").unwrap();
        assert_eq!(scval, ScVal::U32(42));

        let scval_large = parse_key_arg("5000000000").unwrap();
        assert_eq!(scval_large, ScVal::U64(5000000000));
    }

    #[test]
    fn test_parse_key_arg_bool() {
        assert_eq!(parse_key_arg("true").unwrap(), ScVal::Bool(true));
        assert_eq!(parse_key_arg("false").unwrap(), ScVal::Bool(false));
    }

    #[test]
    fn test_parse_key_arg_json_array() {
        let scval = parse_key_arg(r#"["hello", 100]"#).unwrap();
        match scval {
            ScVal::Vec(Some(vec)) => {
                assert_eq!(vec.len(), 2);
                assert_eq!(vec[0], ScVal::Symbol(ScSymbol("hello".try_into().unwrap())));
                assert_eq!(vec[1], ScVal::U32(100));
            }
            other => panic!("expected Vec, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_key_arg_json_object() {
        let scval = parse_key_arg(
            r#"{"admin": "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABSC4"}"#,
        )
        .unwrap();
        match scval {
            ScVal::Map(Some(map)) => {
                assert_eq!(map.len(), 1);
                assert_eq!(
                    map[0].key,
                    ScVal::Symbol(ScSymbol("admin".try_into().unwrap()))
                );
                match &map[0].val {
                    ScVal::Address(ScAddress::Contract(ContractId(Hash(bytes)))) => {
                        assert_eq!(*bytes, [0; 32]);
                    }
                    other => panic!("expected Contract address val, got {:?}", other),
                }
            }
            other => panic!("expected Map, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_key_arg_xdr_json() {
        let scval = parse_key_arg(r#"{"u64": 123456789}"#).unwrap();
        assert_eq!(scval, ScVal::U64(123456789));
    }
}
