use crate::xdr::{ReadXdr, ScVal};

use crate::commands::watch::event::DecodedValue;

pub fn decode_scval(base64: &str) -> DecodedValue {
    match ScVal::from_xdr_base64(base64, crate::xdr::Limits::none()) {
        Ok(val) => DecodedValue {
            display: {
                let s = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    soroban_spec_tools::to_string(&val).unwrap_or_else(|_| base64.to_string())
                }))
                .unwrap_or_else(|_| base64.to_string());
                serde_json::from_str::<String>(&s).unwrap_or(s)
            },
        },
        Err(_) => DecodedValue {
            display: if base64.len() > 20 {
                format!("{}…", &base64[..20])
            } else {
                base64.to_string()
            },
        },
    }
}

/// Shorten a Stellar address/contract ID to `ABCDEFGH..WXYZABCD` format.
pub fn truncate_addr(s: &str) -> String {
    const N: usize = 8;
    if s.len() <= N * 2 + 2 {
        return s.to_string();
    }
    format!("{}..{}", &s[..N], &s[s.len() - N..])
}

pub fn encode_account_key(key: &[u8]) -> String {
    stellar_strkey::ed25519::PublicKey::from_payload(key)
        .map_or_else(|_| hex::encode(key), |k| k.to_string())
}
