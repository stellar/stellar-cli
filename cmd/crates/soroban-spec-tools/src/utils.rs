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
        .or_else(|_| {
            // strkey failed, try to parse it as a hex string, for backwards compatibility.
            padded_hex_from_str(contract_id, 32)
                .map_err(|_| stellar_strkey::DecodeError::Invalid)?
                .try_into()
                .map_err(|_| stellar_strkey::DecodeError::Invalid)
        })
        .map_err(|_| stellar_strkey::DecodeError::Invalid)
}
