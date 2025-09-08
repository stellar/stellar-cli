use clap::Parser;

use crate::{commands::tx, xdr};

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
    /// Balance ID of the claimable balance to clawback. Accepts multiple formats:
    /// - API format with type prefix (72 chars): 000000006f2179b31311fa8064760b48942c8e166702ba0b8fbe7358c4fd570421840461
    /// - Direct hash format (64 chars): 6f2179b31311fa8064760b48942c8e166702ba0b8fbe7358c4fd570421840461
    /// - StrKey format (base32): BAAMLBZI42AD52HKGIZOU7WFVZM6BPEJCLPL44QU2AT6TY3P57I5QDNYIA
    #[arg(long)]
    pub balance_id: String,
}

impl TryFrom<&Cmd> for xdr::OperationBody {
    type Error = tx::args::Error;
    fn try_from(
        Cmd {
            tx: _,
            op: Args { balance_id },
        }: &Cmd,
    ) -> Result<Self, Self::Error> {
        let balance_id_bytes = parse_balance_id(balance_id)?;

        let mut balance_id_array = [0u8; 32];
        balance_id_array.copy_from_slice(&balance_id_bytes);

        let claimable_balance_id =
            xdr::ClaimableBalanceId::ClaimableBalanceIdTypeV0(xdr::Hash(balance_id_array));

        Ok(xdr::OperationBody::ClawbackClaimableBalance(
            xdr::ClawbackClaimableBalanceOp {
                balance_id: claimable_balance_id,
            },
        ))
    }
}

fn parse_balance_id(balance_id: &str) -> Result<Vec<u8>, tx::args::Error> {
    // Handle multiple formats:
    // 1. StrKey format (base32): BAAMLBZI42AD52HKGIZOU7WFVZM6BPEJCLPL44QU2AT6TY3P57I5QDNYIA
    // 2. API format with type prefix (72 hex chars): 000000006f2179b3...
    // 3. Direct hash format (64 hex chars): 6f2179b3...

    if balance_id.starts_with('B') && balance_id.len() > 50 {
        // StrKey format - decode manually since stellar-strkey doesn't have claimable balance type yet
        match decode_strkey_to_hex(balance_id) {
            Ok(hex_string) => {
                // The StrKey decodes to the full API format (with 00000000 prefix)
                // So we need to strip the prefix like we do for API format
                let cleaned_balance_id =
                    if hex_string.len() == 72 && hex_string.starts_with("00000000") {
                        &hex_string[8..]
                    } else {
                        &hex_string
                    };

                let balance_id_bytes =
                    hex::decode(cleaned_balance_id).map_err(|_| tx::args::Error::InvalidHex {
                        name: "balance-id".to_string(),
                        hex: balance_id.to_string(),
                    })?;

                if balance_id_bytes.len() != 32 {
                    return Err(tx::args::Error::InvalidHex {
                        name: "balance-id".to_string(),
                        hex: balance_id.to_string(),
                    });
                }

                Ok(balance_id_bytes)
            }
            Err(_) => Err(tx::args::Error::InvalidHex {
                name: "balance-id".to_string(),
                hex: balance_id.to_string(),
            }),
        }
    } else {
        // Hex format - handle both API format (72 chars) and direct hash (64 chars)
        let cleaned_balance_id = if balance_id.len() == 72 && balance_id.starts_with("00000000") {
            // Remove the 8-character type prefix (00000000 for ClaimableBalanceIdTypeV0)
            &balance_id[8..]
        } else {
            balance_id
        };

        let balance_id_bytes =
            hex::decode(cleaned_balance_id).map_err(|_| tx::args::Error::InvalidHex {
                name: "balance-id".to_string(),
                hex: balance_id.to_string(),
            })?;

        if balance_id_bytes.len() != 32 {
            return Err(tx::args::Error::InvalidHex {
                name: "balance-id".to_string(),
                hex: balance_id.to_string(),
            });
        }

        Ok(balance_id_bytes)
    }
}

fn decode_strkey_to_hex(strkey: &str) -> Result<String, String> {
    // Claimable balance IDs are not a recognized StrKey type in stellar-strkey,
    // so we decode them manually using base32
    decode_strkey_manually(strkey)
}

fn decode_strkey_manually(strkey: &str) -> Result<String, String> {
    use base32::{decode, Alphabet};

    // StrKey uses RFC4648 alphabet without padding
    let alphabet = Alphabet::Rfc4648 { padding: false };

    // Decode the base32 string
    let bytes = decode(alphabet, strkey).ok_or_else(|| "Failed to decode base32".to_string())?;

    // Validate minimum length (version + payload + crc)
    if bytes.len() < 3 {
        return Err(format!("StrKey too short: {} bytes", bytes.len()));
    }

    // Extract the payload (skip 1-byte version, exclude 2-byte CRC)
    let payload_end = bytes.len() - 2; // Exclude 2-byte CRC
    let mut payload = &bytes[1..payload_end];

    // If payload starts with 00 (claimable balance type indicator), skip it to get the 32-byte hash
    if payload.len() == 33 && payload[0] == 0x00 {
        payload = &payload[1..]; // Skip the leading 00 to get the actual 32-byte hash
    }

    // Return with claimable balance type prefix (00000000)
    Ok(format!("00000000{}", hex::encode(payload)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_balance_id_hex_parsing() {
        let balance_id = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let balance_id_bytes = hex::decode(balance_id).unwrap();
        assert_eq!(balance_id_bytes.len(), 32);

        let mut balance_id_array = [0u8; 32];
        balance_id_array.copy_from_slice(&balance_id_bytes);

        let claimable_balance_id =
            xdr::ClaimableBalanceId::ClaimableBalanceIdTypeV0(xdr::Hash(balance_id_array));

        let op = xdr::ClawbackClaimableBalanceOp {
            balance_id: claimable_balance_id,
        };

        let xdr::ClaimableBalanceId::ClaimableBalanceIdTypeV0(hash) = op.balance_id;
        assert_eq!(hash.0.to_vec(), balance_id_bytes);
    }

    #[test]
    fn test_api_format_with_prefix() {
        let api_format_id =
            "000000006f2179b31311fa8064760b48942c8e166702ba0b8fbe7358c4fd570421840461";
        let expected_hash = "6f2179b31311fa8064760b48942c8e166702ba0b8fbe7358c4fd570421840461";

        // Test that we correctly strip the prefix
        let cleaned_id = if api_format_id.len() == 72 && api_format_id.starts_with("00000000") {
            &api_format_id[8..]
        } else {
            api_format_id
        };

        assert_eq!(cleaned_id, expected_hash);
        assert_eq!(cleaned_id.len(), 64);

        let balance_id_bytes = hex::decode(cleaned_id).unwrap();
        assert_eq!(balance_id_bytes.len(), 32);
    }

    #[test]
    fn test_direct_hash_format() {
        let direct_format_id = "6f2179b31311fa8064760b48942c8e166702ba0b8fbe7358c4fd570421840461";

        // Test that direct format passes through unchanged
        let cleaned_id = if direct_format_id.len() == 72 && direct_format_id.starts_with("00000000")
        {
            &direct_format_id[8..]
        } else {
            direct_format_id
        };

        assert_eq!(cleaned_id, direct_format_id);
        assert_eq!(cleaned_id.len(), 64);

        let balance_id_bytes = hex::decode(cleaned_id).unwrap();
        assert_eq!(balance_id_bytes.len(), 32);
    }

    #[test]
    fn test_invalid_balance_id_too_short() {
        let balance_id = "0123456789abcdef";
        let balance_id_bytes = hex::decode(balance_id).unwrap();
        assert_ne!(balance_id_bytes.len(), 32);
    }

    #[test]
    fn test_invalid_balance_id_too_long() {
        let balance_id = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef00";
        let balance_id_bytes = hex::decode(balance_id).unwrap();
        assert_ne!(balance_id_bytes.len(), 32);
    }

    #[test]
    fn test_strkey_format() {
        let strkey_id = "BAAMLBZI42AD52HKGIZOU7WFVZM6BPEJCLPL44QU2AT6TY3P57I5QDNYIA";
        let expected_hex = "c58728e6803ee8ea3232ea7ec5ae59e0bc8912debe7214d027e9e36fefd1d80d";

        // Test that StrKey format can be decoded
        let result = parse_balance_id(strkey_id);
        assert!(result.is_ok(), "StrKey format should decode successfully");

        let bytes = result.unwrap();
        assert_eq!(bytes.len(), 32, "Should decode to 32 bytes");
        assert_eq!(
            hex::encode(&bytes),
            expected_hex,
            "Should match expected hex"
        );
    }

    #[test]
    fn test_decode_strkey_manually() {
        let strkey = "BAAMLBZI42AD52HKGIZOU7WFVZM6BPEJCLPL44QU2AT6TY3P57I5QDNYIA";
        let expected = "00000000c58728e6803ee8ea3232ea7ec5ae59e0bc8912debe7214d027e9e36fefd1d80d";

        let result = decode_strkey_manually(strkey);
        assert!(
            result.is_ok(),
            "Should decode StrKey successfully: {:?}",
            result
        );
        let actual = result.unwrap();
        assert_eq!(actual, expected, "Should match expected API format");
    }

    #[test]
    fn test_invalid_balance_id_not_hex() {
        let balance_id = "not_hex_characters_here_not_valid_at_all_exactly_64_chars";
        let result = hex::decode(balance_id);
        assert!(result.is_err());
    }
}
