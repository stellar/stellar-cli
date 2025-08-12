use clap::{command, Parser};
use serde_json;
use std::str::FromStr;

use crate::{commands::tx, config::address, tx::builder, xdr};

#[derive(Debug, serde::Deserialize, serde::Serialize, PartialEq)]
#[serde(rename_all = "snake_case", tag = "type", content = "value")]
pub enum PredicateJson {
    Unconditional,
    BeforeAbsoluteTime(String),
    BeforeRelativeTime(u64),
    Not(Box<PredicateJson>),
    And(Vec<PredicateJson>),
    Or(Vec<PredicateJson>),
}

fn parse_claimant_string(input: &str) -> Result<(String, Option<PredicateJson>), String> {
    if let Some((account, predicate_str)) = input.split_once(':') {
        let predicate: PredicateJson = serde_json::from_str(predicate_str)
            .map_err(|e| format!("Invalid predicate JSON: {}", e))?;
        Ok((account.to_string(), Some(predicate)))
    } else {
        Ok((input.to_string(), None))
    }
}

fn predicate_json_to_xdr(predicate: &PredicateJson) -> Result<xdr::ClaimPredicate, String> {
    match predicate {
        PredicateJson::Unconditional => Ok(xdr::ClaimPredicate::Unconditional),
        PredicateJson::BeforeAbsoluteTime(time_str) => {
            // Parse ISO8601 timestamp to Unix timestamp
            let timestamp = chrono::DateTime::parse_from_rfc3339(time_str)
                .map_err(|e| format!("Invalid timestamp format: {}", e))?
                .timestamp() as u64;
            Ok(xdr::ClaimPredicate::BeforeAbsoluteTime(timestamp as i64))
        }
        PredicateJson::BeforeRelativeTime(seconds) => {
            Ok(xdr::ClaimPredicate::BeforeRelativeTime(*seconds as i64))
        }
        PredicateJson::Not(inner) => {
            let inner_predicate = predicate_json_to_xdr(inner)?;
            Ok(xdr::ClaimPredicate::Not(Some(Box::new(inner_predicate))))
        }
        PredicateJson::And(predicates) => {
            if predicates.len() != 2 {
                return Err("And predicate must have exactly 2 sub-predicates".to_string());
            }
            let left = predicate_json_to_xdr(&predicates[0])?;
            let right = predicate_json_to_xdr(&predicates[1])?;
            let vec_m = xdr::VecM::try_from(vec![left, right])
                .map_err(|_| "Failed to create VecM for And predicate".to_string())?;
            Ok(xdr::ClaimPredicate::And(vec_m))
        }
        PredicateJson::Or(predicates) => {
            if predicates.len() != 2 {
                return Err("Or predicate must have exactly 2 sub-predicates".to_string());
            }
            let left = predicate_json_to_xdr(&predicates[0])?;
            let right = predicate_json_to_xdr(&predicates[1])?;
            let vec_m = xdr::VecM::try_from(vec![left, right])
                .map_err(|_| "Failed to create VecM for Or predicate".to_string())?;
            Ok(xdr::ClaimPredicate::Or(vec_m))
        }
    }
}

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub tx: tx::Args,

    #[clap(flatten)]
    pub op: Args,
}

#[derive(Debug, clap::Args, Clone)]
pub struct Args {
    /// Asset to be held in the ClaimableBalanceEntry
    #[arg(long, default_value = "native")]
    pub asset: builder::Asset,

    /// Amount of asset to store in the entry, in stroops. 1 stroop = 0.0000001 of the asset.
    #[arg(long)]
    pub amount: builder::Amount,

    /// Claimants of the claimable balance. Format: account_id or account_id:predicate_json
    /// Can be specified multiple times for multiple claimants.
    /// Examples:
    /// - --claimant alice (unconditional)
    /// - --claimant 'bob:{"type":"before_absolute_time","value":"2024-12-31T23:59:59Z"}'
    /// - --claimant 'charlie:{"type":"and","value":[{"type":"before_absolute_time","value":"2024-12-31T23:59:59Z"},{"type":"before_relative_time","value":3600}]}'
    #[arg(long = "claimant", action = clap::ArgAction::Append)]
    pub claimants: Vec<String>,
}

impl TryFrom<&Cmd> for xdr::OperationBody {
    type Error = tx::args::Error;
    fn try_from(
        Cmd {
            tx,
            op:
                Args {
                    asset,
                    amount,
                    claimants,
                },
        }: &Cmd,
    ) -> Result<Self, Self::Error> {
        let claimants_vec = claimants
            .iter()
            .map(|claimant_str| {
                let (account_str, predicate_json) =
                    parse_claimant_string(claimant_str).map_err(|e| {
                        tx::args::Error::Address(address::Error::InvalidKeyNameLength(e))
                    })?;

                let account_address = address::UnresolvedMuxedAccount::from_str(&account_str)
                    .map_err(|e| tx::args::Error::Address(e))?;
                let muxed_account = tx.resolve_muxed_address(&account_address)?;

                let predicate = match predicate_json {
                    Some(predicate) => predicate_json_to_xdr(&predicate).map_err(|e| {
                        tx::args::Error::Address(address::Error::InvalidKeyNameLength(e))
                    })?,
                    None => xdr::ClaimPredicate::Unconditional,
                };

                Ok(xdr::Claimant::ClaimantTypeV0(xdr::ClaimantV0 {
                    destination: muxed_account.account_id(),
                    predicate,
                }))
            })
            .collect::<Result<Vec<_>, tx::args::Error>>()?;

        Ok(xdr::OperationBody::CreateClaimableBalance(
            xdr::CreateClaimableBalanceOp {
                asset: tx.resolve_asset(asset)?,
                amount: amount.into(),
                claimants: claimants_vec.try_into().map_err(|_| {
                    tx::args::Error::Address(address::Error::InvalidKeyNameLength(
                        "Too many claimants".to_string(),
                    ))
                })?,
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_claimant_string_unconditional() {
        let result =
            parse_claimant_string("GCNV6VMPZNHQTACVZC4AE75SJAFLHP7USOQWGE2HWMLXDKP6XOLGJR7S");
        assert_eq!(
            result,
            Ok((
                "GCNV6VMPZNHQTACVZC4AE75SJAFLHP7USOQWGE2HWMLXDKP6XOLGJR7S".to_string(),
                None
            ))
        );
    }

    #[test]
    fn test_parse_claimant_string_explicit_unconditional() {
        let input =
            r#"GCNV6VMPZNHQTACVZC4AE75SJAFLHP7USOQWGE2HWMLXDKP6XOLGJR7S:{"type":"unconditional"}"#;
        let result = parse_claimant_string(input);
        assert_eq!(
            result,
            Ok((
                "GCNV6VMPZNHQTACVZC4AE75SJAFLHP7USOQWGE2HWMLXDKP6XOLGJR7S".to_string(),
                Some(PredicateJson::Unconditional)
            ))
        );
    }

    #[test]
    fn test_parse_claimant_string_before_absolute_time() {
        let input = r#"GCNV6VMPZNHQTACVZC4AE75SJAFLHP7USOQWGE2HWMLXDKP6XOLGJR7S:{"type":"before_absolute_time","value":"2024-12-31T23:59:59Z"}"#;
        let result = parse_claimant_string(input);
        assert_eq!(
            result,
            Ok((
                "GCNV6VMPZNHQTACVZC4AE75SJAFLHP7USOQWGE2HWMLXDKP6XOLGJR7S".to_string(),
                Some(PredicateJson::BeforeAbsoluteTime(
                    "2024-12-31T23:59:59Z".to_string()
                ))
            ))
        );
    }

    #[test]
    fn test_parse_claimant_string_before_relative_time() {
        let input = r#"GCNV6VMPZNHQTACVZC4AE75SJAFLHP7USOQWGE2HWMLXDKP6XOLGJR7S:{"type":"before_relative_time","value":3600}"#;
        let result = parse_claimant_string(input);
        assert_eq!(
            result,
            Ok((
                "GCNV6VMPZNHQTACVZC4AE75SJAFLHP7USOQWGE2HWMLXDKP6XOLGJR7S".to_string(),
                Some(PredicateJson::BeforeRelativeTime(3600))
            ))
        );
    }

    #[test]
    fn test_parse_claimant_string_not_predicate() {
        let input = r#"GCNV6VMPZNHQTACVZC4AE75SJAFLHP7USOQWGE2HWMLXDKP6XOLGJR7S:{"type":"not","value":{"type":"before_relative_time","value":3600}}"#;
        let result = parse_claimant_string(input);
        assert_eq!(
            result,
            Ok((
                "GCNV6VMPZNHQTACVZC4AE75SJAFLHP7USOQWGE2HWMLXDKP6XOLGJR7S".to_string(),
                Some(PredicateJson::Not(Box::new(
                    PredicateJson::BeforeRelativeTime(3600)
                )))
            ))
        );
    }

    #[test]
    fn test_parse_claimant_string_and_predicate() {
        let input = r#"GCNV6VMPZNHQTACVZC4AE75SJAFLHP7USOQWGE2HWMLXDKP6XOLGJR7S:{"type":"and","value":[{"type":"before_absolute_time","value":"2024-12-31T23:59:59Z"},{"type":"before_relative_time","value":7200}]}"#;
        let result = parse_claimant_string(input);
        assert_eq!(
            result,
            Ok((
                "GCNV6VMPZNHQTACVZC4AE75SJAFLHP7USOQWGE2HWMLXDKP6XOLGJR7S".to_string(),
                Some(PredicateJson::And(vec![
                    PredicateJson::BeforeAbsoluteTime("2024-12-31T23:59:59Z".to_string()),
                    PredicateJson::BeforeRelativeTime(7200)
                ]))
            ))
        );
    }

    #[test]
    fn test_parse_claimant_string_or_predicate() {
        let input = r#"GCNV6VMPZNHQTACVZC4AE75SJAFLHP7USOQWGE2HWMLXDKP6XOLGJR7S:{"type":"or","value":[{"type":"before_absolute_time","value":"2024-12-31T23:59:59Z"},{"type":"unconditional"}]}"#;
        let result = parse_claimant_string(input);
        assert_eq!(
            result,
            Ok((
                "GCNV6VMPZNHQTACVZC4AE75SJAFLHP7USOQWGE2HWMLXDKP6XOLGJR7S".to_string(),
                Some(PredicateJson::Or(vec![
                    PredicateJson::BeforeAbsoluteTime("2024-12-31T23:59:59Z".to_string()),
                    PredicateJson::Unconditional
                ]))
            ))
        );
    }

    #[test]
    fn test_parse_claimant_string_invalid_json() {
        let input = r#"GCNV6VMPZNHQTACVZC4AE75SJAFLHP7USOQWGE2HWMLXDKP6XOLGJR7S:{"invalid": json}"#;
        let result = parse_claimant_string(input);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid predicate JSON"));
    }

    #[test]
    fn test_predicate_json_to_xdr_unconditional() {
        let predicate = PredicateJson::Unconditional;
        let result = predicate_json_to_xdr(&predicate);
        assert_eq!(result, Ok(xdr::ClaimPredicate::Unconditional));
    }

    #[test]
    fn test_predicate_json_to_xdr_before_absolute_time() {
        let predicate = PredicateJson::BeforeAbsoluteTime("2024-12-31T23:59:59Z".to_string());
        let result = predicate_json_to_xdr(&predicate);
        assert!(result.is_ok());
        match result.unwrap() {
            xdr::ClaimPredicate::BeforeAbsoluteTime(timestamp) => {
                assert_eq!(timestamp, 1735689599); // Unix timestamp for 2024-12-31T23:59:59Z
            }
            _ => panic!("Expected BeforeAbsoluteTime predicate"),
        }
    }

    #[test]
    fn test_predicate_json_to_xdr_before_relative_time() {
        let predicate = PredicateJson::BeforeRelativeTime(3600);
        let result = predicate_json_to_xdr(&predicate);
        assert!(result.is_ok());
        match result.unwrap() {
            xdr::ClaimPredicate::BeforeRelativeTime(seconds) => {
                assert_eq!(seconds, 3600);
            }
            _ => panic!("Expected BeforeRelativeTime predicate"),
        }
    }

    #[test]
    fn test_predicate_json_to_xdr_not() {
        let predicate = PredicateJson::Not(Box::new(PredicateJson::BeforeRelativeTime(3600)));
        let result = predicate_json_to_xdr(&predicate);
        assert!(result.is_ok());
        match result.unwrap() {
            xdr::ClaimPredicate::Not(Some(inner)) => match *inner {
                xdr::ClaimPredicate::BeforeRelativeTime(seconds) => {
                    assert_eq!(seconds, 3600);
                }
                _ => panic!("Expected BeforeRelativeTime inside Not predicate"),
            },
            _ => panic!("Expected Not predicate"),
        }
    }

    #[test]
    fn test_predicate_json_to_xdr_and() {
        let predicate = PredicateJson::And(vec![
            PredicateJson::BeforeAbsoluteTime("2024-12-31T23:59:59Z".to_string()),
            PredicateJson::BeforeRelativeTime(7200),
        ]);
        let result = predicate_json_to_xdr(&predicate);
        assert!(result.is_ok());
        match result.unwrap() {
            xdr::ClaimPredicate::And(predicates) => {
                assert_eq!(predicates.len(), 2);
            }
            _ => panic!("Expected And predicate"),
        }
    }

    #[test]
    fn test_predicate_json_to_xdr_or() {
        let predicate = PredicateJson::Or(vec![
            PredicateJson::BeforeAbsoluteTime("2024-12-31T23:59:59Z".to_string()),
            PredicateJson::Unconditional,
        ]);
        let result = predicate_json_to_xdr(&predicate);
        assert!(result.is_ok());
        match result.unwrap() {
            xdr::ClaimPredicate::Or(predicates) => {
                assert_eq!(predicates.len(), 2);
            }
            _ => panic!("Expected Or predicate"),
        }
    }

    #[test]
    fn test_predicate_json_to_xdr_and_wrong_count() {
        let predicate = PredicateJson::And(vec![PredicateJson::Unconditional]);
        let result = predicate_json_to_xdr(&predicate);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("And predicate must have exactly 2 sub-predicates"));
    }

    #[test]
    fn test_predicate_json_to_xdr_or_wrong_count() {
        let predicate = PredicateJson::Or(vec![
            PredicateJson::Unconditional,
            PredicateJson::BeforeRelativeTime(3600),
            PredicateJson::BeforeAbsoluteTime("2024-12-31T23:59:59Z".to_string()),
        ]);
        let result = predicate_json_to_xdr(&predicate);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("Or predicate must have exactly 2 sub-predicates"));
    }

    #[test]
    fn test_predicate_json_to_xdr_invalid_timestamp() {
        let predicate = PredicateJson::BeforeAbsoluteTime("invalid-timestamp".to_string());
        let result = predicate_json_to_xdr(&predicate);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid timestamp format"));
    }
}
