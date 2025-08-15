use clap::{command, Parser};
use serde_json;
use std::str::FromStr;

use crate::{commands::tx, config::address, tx::builder, xdr};

fn parse_claimant_string(input: &str) -> Result<(String, Option<xdr::ClaimPredicate>), String> {
    if let Some((account, predicate_str)) = input.split_once(':') {
        let predicate: xdr::ClaimPredicate = serde_json::from_str(predicate_str)
            .map_err(|e| format!("Invalid predicate JSON: {e}"))?;
        Ok((account.to_string(), Some(predicate)))
    } else {
        Ok((input.to_string(), None))
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
    /// - --claimant 'bob:{"before_absolute_time":"1735689599"}'
    /// - --claimant 'charlie:{"and":[{"before_absolute_time":"1735689599"},{"before_relative_time":"3600"}]}'
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
                let (account_str, predicate) =
                    parse_claimant_string(claimant_str).map_err(|e| {
                        tx::args::Error::Address(address::Error::InvalidKeyNameLength(e))
                    })?;

                let account_address = address::UnresolvedMuxedAccount::from_str(&account_str)
                    .map_err(tx::args::Error::Address)?;
                let muxed_account = tx.resolve_muxed_address(&account_address)?;

                let predicate = predicate.unwrap_or(xdr::ClaimPredicate::Unconditional);

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
        let input = r#"GCNV6VMPZNHQTACVZC4AE75SJAFLHP7USOQWGE2HWMLXDKP6XOLGJR7S:"unconditional""#;
        let result = parse_claimant_string(input);
        assert_eq!(
            result,
            Ok((
                "GCNV6VMPZNHQTACVZC4AE75SJAFLHP7USOQWGE2HWMLXDKP6XOLGJR7S".to_string(),
                Some(xdr::ClaimPredicate::Unconditional)
            ))
        );
    }

    #[test]
    fn test_parse_claimant_string_before_absolute_time() {
        let input = r#"GCNV6VMPZNHQTACVZC4AE75SJAFLHP7USOQWGE2HWMLXDKP6XOLGJR7S:{"before_absolute_time":"1735689599"}"#;
        let result = parse_claimant_string(input);
        assert_eq!(
            result,
            Ok((
                "GCNV6VMPZNHQTACVZC4AE75SJAFLHP7USOQWGE2HWMLXDKP6XOLGJR7S".to_string(),
                Some(xdr::ClaimPredicate::BeforeAbsoluteTime(1_735_689_599))
            ))
        );
    }

    #[test]
    fn test_parse_claimant_string_before_relative_time() {
        let input = r#"GCNV6VMPZNHQTACVZC4AE75SJAFLHP7USOQWGE2HWMLXDKP6XOLGJR7S:{"before_relative_time":"3600"}"#;
        let result = parse_claimant_string(input);
        assert_eq!(
            result,
            Ok((
                "GCNV6VMPZNHQTACVZC4AE75SJAFLHP7USOQWGE2HWMLXDKP6XOLGJR7S".to_string(),
                Some(xdr::ClaimPredicate::BeforeRelativeTime(3600))
            ))
        );
    }

    #[test]
    fn test_parse_claimant_string_not_predicate() {
        let input = r#"GCNV6VMPZNHQTACVZC4AE75SJAFLHP7USOQWGE2HWMLXDKP6XOLGJR7S:{"not":{"before_relative_time":"3600"}}"#;
        let result = parse_claimant_string(input);
        assert!(result.is_ok());
        let (_, predicate) = result.unwrap();
        match predicate {
            Some(xdr::ClaimPredicate::Not(Some(inner))) => match inner.as_ref() {
                xdr::ClaimPredicate::BeforeRelativeTime(3600) => {}
                _ => panic!("Expected BeforeRelativeTime inside Not"),
            },
            _ => panic!("Expected Not predicate"),
        }
    }

    #[test]
    fn test_parse_claimant_string_and_predicate() {
        let input = r#"GCNV6VMPZNHQTACVZC4AE75SJAFLHP7USOQWGE2HWMLXDKP6XOLGJR7S:{"and":[{"before_absolute_time":"1735689599"},{"before_relative_time":"7200"}]}"#;
        let result = parse_claimant_string(input);
        assert!(result.is_ok());
        let (_, predicate) = result.unwrap();
        match predicate {
            Some(xdr::ClaimPredicate::And(predicates)) => {
                assert_eq!(predicates.len(), 2);
            }
            _ => panic!("Expected And predicate"),
        }
    }

    #[test]
    fn test_parse_claimant_string_or_predicate() {
        let input = r#"GCNV6VMPZNHQTACVZC4AE75SJAFLHP7USOQWGE2HWMLXDKP6XOLGJR7S:{"or":[{"before_absolute_time":"1735689599"},"unconditional"]}"#;
        let result = parse_claimant_string(input);
        assert!(result.is_ok());
        let (_, predicate) = result.unwrap();
        match predicate {
            Some(xdr::ClaimPredicate::Or(predicates)) => {
                assert_eq!(predicates.len(), 2);
            }
            _ => panic!("Expected Or predicate"),
        }
    }

    #[test]
    fn test_parse_claimant_string_invalid_json() {
        let input = r#"GCNV6VMPZNHQTACVZC4AE75SJAFLHP7USOQWGE2HWMLXDKP6XOLGJR7S:{"invalid": json}"#;
        let result = parse_claimant_string(input);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid predicate JSON"));
    }
}
