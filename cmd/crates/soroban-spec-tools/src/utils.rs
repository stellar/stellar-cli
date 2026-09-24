use hex::FromHexError;

/// # Errors
///
/// Might return an error
pub fn padded_hex_from_str(s: &str, n: usize) -> Result<Vec<u8>, FromHexError> {
    if s.len() > n * 2 {
        return Err(FromHexError::InvalidStringLength);
    }
    let mut decoded = vec![0u8; n];
    let padded = format!("{s:0>width$}", width = n * 2);
    hex::decode_to_slice(padded, &mut decoded)?;
    Ok(decoded)
}

/// # Errors
///
/// Might return an error
pub fn contract_id_from_str(contract_id: &str) -> Result<[u8; 32], stellar_strkey::DecodeError> {
    stellar_strkey::Contract::from_string(contract_id)
        .map(|strkey| strkey.0)
        .or_else(|e| {
            // strkey failed, try to parse it as a hex string, for backwards compatibility.
            // If that also fails, report the strkey error, since strkey is the expected format.
            padded_hex_from_str(contract_id, 32)
                .ok()
                .and_then(|bytes| bytes.try_into().ok())
                .ok_or(e)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contract_id_from_str_strkey() {
        let id = stellar_strkey::Contract([1; 32]);
        assert_eq!(contract_id_from_str(&id.to_string()), Ok([1; 32]));
    }

    #[test]
    fn contract_id_from_str_hex() {
        assert_eq!(contract_id_from_str(&"01".repeat(32)), Ok([1; 32]));
        let mut expected = [0; 32];
        expected[31] = 1;
        assert_eq!(contract_id_from_str("1"), Ok(expected));
    }

    #[test]
    fn contract_id_from_str_returns_strkey_error() {
        // Change a character in the payload so that the checksum no longer matches.
        let id = stellar_strkey::Contract([1; 32]).to_string();
        let id = format!("{}B{}", &id[..10], &id[11..]);
        assert_ne!(id, stellar_strkey::Contract([1; 32]).to_string().as_str());
        assert_eq!(
            contract_id_from_str(&id),
            Err(stellar_strkey::DecodeError::ChecksumMismatch)
        );
    }
}
