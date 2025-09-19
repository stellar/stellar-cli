use clap::Parser;
use soroban_sdk::xdr;

use super::clawback_claimable_balance::parse_balance_id;
use crate::{commands::tx, config::address, tx::builder};

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub tx: tx::args::Args,

    #[command(flatten)]
    pub op: Args,
}

#[derive(Debug, clap::Args, Clone)]
pub struct Args {
    /// Account ID (required for all sponsorship types)
    #[arg(long)]
    pub account_id: address::UnresolvedMuxedAccount,

    /// Asset for trustline sponsorship (format: CODE:ISSUER or native)
    #[arg(long, group = "sponsorship_type")]
    pub asset: Option<builder::Asset>,

    /// Data name for data entry sponsorship
    #[arg(long, group = "sponsorship_type")]
    pub data_name: Option<String>,

    /// Offer ID for offer sponsorship
    #[arg(long, group = "sponsorship_type")]
    pub offer_id: Option<u64>,

    /// Pool ID for liquidity pool sponsorship. Accepts multiple formats:
    /// - API format with type prefix (72 chars): 000000006f2179b31311fa8064760b48942c8e166702ba0b8fbe7358c4fd570421840461
    /// - Direct hash format (64 chars): 6f2179b31311fa8064760b48942c8e166702ba0b8fbe7358c4fd570421840461
    /// - StrKey format (base32): LAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
    #[arg(long, group = "sponsorship_type")]
    pub liquidity_pool_id: Option<String>,

    /// Claimable balance ID for claimable balance sponsorship. Accepts multiple formats:
    /// - API format with type prefix (72 chars): 000000006f2179b31311fa8064760b48942c8e166702ba0b8fbe7358c4fd570421840461
    /// - Direct hash format (64 chars): 6f2179b31311fa8064760b48942c8e166702ba0b8fbe7358c4fd570421840461
    /// - StrKey format (base32): BAAMLBZI42AD52HKGIZOU7WFVZM6BPEJCLPL44QU2AT6TY3P57I5QDNYIA
    #[arg(long, group = "sponsorship_type")]
    pub claimable_balance_id: Option<String>,

    /// Signer key for signer sponsorship
    #[arg(long, group = "sponsorship_type")]
    pub signer_key: Option<address::UnresolvedMuxedAccount>,
}

fn parse_liquidity_pool_id(pool_id: &str) -> Result<Vec<u8>, tx::args::Error> {
    // Handle multiple formats:
    // 1. StrKey format (base32): LAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
    // 2. API format with type prefix (72 hex chars): 000000006f2179b3...
    // 3. Direct hash format (64 hex chars): 6f2179b3...

    if pool_id.starts_with('L') && pool_id.len() > 50 {
        match stellar_strkey::Strkey::from_string(pool_id) {
            Ok(stellar_strkey::Strkey::LiquidityPool(pool)) => Ok(pool.0.to_vec()),
            _ => Err(tx::args::Error::InvalidHex {
                name: "liquidity_pool_id".to_string(),
                hex: pool_id.to_string(),
            }),
        }
    } else {
        let cleaned_pool_id = if pool_id.len() == 72 && pool_id.starts_with("00000000") {
            &pool_id[8..]
        } else {
            pool_id
        };

        let pool_id_bytes =
            hex::decode(cleaned_pool_id).map_err(|_| tx::args::Error::InvalidHex {
                name: "liquidity_pool_id".to_string(),
                hex: pool_id.to_string(),
            })?;

        if pool_id_bytes.len() != 32 {
            return Err(tx::args::Error::InvalidHex {
                name: "liquidity_pool_id".to_string(),
                hex: pool_id.to_string(),
            });
        }

        Ok(pool_id_bytes)
    }
}

impl TryFrom<&Cmd> for xdr::OperationBody {
    type Error = tx::args::Error;
    fn try_from(cmd: &Cmd) -> Result<Self, Self::Error> {
        let account_id_key = cmd.tx.resolve_account_id(&cmd.op.account_id)?;

        let revoke_op = if let Some(signer_key) = &cmd.op.signer_key {
            // Signer sponsorship
            let resolved_account = cmd.tx.resolve_account_id(signer_key)?;
            let signer_key = match resolved_account.0 {
                xdr::PublicKey::PublicKeyTypeEd25519(uint256) => xdr::SignerKey::Ed25519(uint256),
            };
            xdr::RevokeSponsorshipOp::Signer(xdr::RevokeSponsorshipOpSigner {
                account_id: account_id_key,
                signer_key,
            })
        } else if let Some(asset) = &cmd.op.asset {
            // Trustline sponsorship
            let resolved_asset = cmd.tx.resolve_asset(asset)?;
            let trustline_asset = match resolved_asset {
                xdr::Asset::CreditAlphanum4(asset) => xdr::TrustLineAsset::CreditAlphanum4(asset),
                xdr::Asset::CreditAlphanum12(asset) => xdr::TrustLineAsset::CreditAlphanum12(asset),
                xdr::Asset::Native => xdr::TrustLineAsset::Native,
            };
            let ledger_key = xdr::LedgerKey::Trustline(xdr::LedgerKeyTrustLine {
                account_id: account_id_key,
                asset: trustline_asset,
            });
            xdr::RevokeSponsorshipOp::LedgerEntry(ledger_key)
        } else if let Some(data_name) = &cmd.op.data_name {
            // Data entry sponsorship
            let data_name_xdr: xdr::StringM<64> =
                data_name.parse().map_err(|_| tx::args::Error::InvalidHex {
                    name: "data_name".to_string(),
                    hex: "invalid data name".to_string(),
                })?;
            let ledger_key = xdr::LedgerKey::Data(xdr::LedgerKeyData {
                account_id: account_id_key,
                data_name: data_name_xdr.into(),
            });
            xdr::RevokeSponsorshipOp::LedgerEntry(ledger_key)
        } else if let Some(offer_id) = cmd.op.offer_id {
            // Offer sponsorship
            let ledger_key = xdr::LedgerKey::Offer(xdr::LedgerKeyOffer {
                seller_id: account_id_key,
                offer_id: offer_id
                    .try_into()
                    .map_err(|_| tx::args::Error::InvalidHex {
                        name: "offer_id".to_string(),
                        hex: "offer ID too large".to_string(),
                    })?,
            });
            xdr::RevokeSponsorshipOp::LedgerEntry(ledger_key)
        } else if let Some(claimable_balance_id) = &cmd.op.claimable_balance_id {
            // Claimable balance sponsorship
            let balance_id_bytes = parse_balance_id(claimable_balance_id)?;
            let mut balance_id_array = [0u8; 32];
            balance_id_array.copy_from_slice(&balance_id_bytes);
            let claimable_balance_id_xdr =
                xdr::ClaimableBalanceId::ClaimableBalanceIdTypeV0(xdr::Hash(balance_id_array));
            let ledger_key = xdr::LedgerKey::ClaimableBalance(xdr::LedgerKeyClaimableBalance {
                balance_id: claimable_balance_id_xdr,
            });
            xdr::RevokeSponsorshipOp::LedgerEntry(ledger_key)
        } else if let Some(liquidity_pool_id) = &cmd.op.liquidity_pool_id {
            // Liquidity pool sponsorship
            let pool_id_bytes = parse_liquidity_pool_id(liquidity_pool_id)?;
            let pool_id_array: [u8; 32] =
                pool_id_bytes
                    .try_into()
                    .map_err(|_| tx::args::Error::InvalidHex {
                        name: "liquidity_pool_id".to_string(),
                        hex: "must be 32 bytes".to_string(),
                    })?;
            let ledger_key = xdr::LedgerKey::LiquidityPool(xdr::LedgerKeyLiquidityPool {
                liquidity_pool_id: xdr::PoolId(xdr::Hash(pool_id_array)),
            });
            xdr::RevokeSponsorshipOp::LedgerEntry(ledger_key)
        } else {
            // Account sponsorship (default when no other specific args provided)
            let ledger_key = xdr::LedgerKey::Account(xdr::LedgerKeyAccount {
                account_id: account_id_key,
            });
            xdr::RevokeSponsorshipOp::LedgerEntry(ledger_key)
        };

        Ok(xdr::OperationBody::RevokeSponsorship(revoke_op))
    }
}
