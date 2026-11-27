//! The checkout spend cap.
//!
//! `--max-amount` is in the asset's base units (stroops for a Stellar Asset
//! Contract), the same units the `stellar token` commands take, and `none` /
//! `off` / `unlimited` disables it. `--max-decimal-amount` is the same cap in
//! the asset's decimal units (`1` = 1.0 of the asset). With neither, the cap is
//! 1 whole unit of the asset, so an unconfigured `checkout` refuses to pay more
//! than that.

use std::str::FromStr;

use crate::commands::token::args::MAX_DECIMALS;

/// The `--max-decimal-amount` used when no cap is given: 1 whole unit of the asset.
pub const DEFAULT_DECIMAL_AMOUNT: &str = "1";

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(
        "invalid --max-amount `{0}`: expected an amount in the asset's base units, \
         or `none` to disable"
    )]
    Invalid(String),

    #[error("invalid --max-decimal-amount `{0}`: expected a decimal amount")]
    InvalidDecimal(String),

    #[error(
        "--max-decimal-amount `{amount}` has more fractional digits than the asset's \
         {decimals} decimals"
    )]
    TooPrecise { amount: String, decimals: u32 },

    #[error("--max-decimal-amount `{0}` is too large for this asset")]
    Overflow(String),

    #[error(
        "the payment requires {required} but the spend cap is {cap} \
         (asset's base units).\n\
         Raise it with `--max-amount <BASE_UNITS>` or `--max-decimal-amount <AMOUNT>`, \
         or disable it with `--max-amount none`."
    )]
    Exceeded { required: i128, cap: i128 },

    #[error("the asset's {decimals} decimals exceed the supported maximum of {MAX_DECIMALS}")]
    DecimalsTooLarge { decimals: u32 },
}

impl Error {
    /// Machine-readable discriminator for the JSON error envelope's `type` field.
    #[must_use]
    pub fn error_type(&self) -> &'static str {
        match self {
            Error::Invalid(_) => "invalid_max_amount",
            Error::InvalidDecimal(_) => "invalid_max_decimal_amount",
            Error::TooPrecise { .. } => "max_amount_too_precise",
            Error::Overflow(_) => "max_amount_overflow",
            Error::Exceeded { .. } => "max_amount_exceeded",
            Error::DecimalsTooLarge { .. } => "asset_decimals_too_large",
        }
    }
}

/// The `--max-amount` cap: a limit in the asset's base units, or no limit at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaxAmount {
    /// No cap — pay whatever the challenge asks.
    Unlimited,
    /// The most to pay, in the asset's base units.
    Limit(i128),
}

impl FromStr for MaxAmount {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let trimmed = s.trim();
        match trimmed.to_ascii_lowercase().as_str() {
            "none" | "off" | "unlimited" => Ok(MaxAmount::Unlimited),
            _ => match trimmed.parse::<i128>() {
                Ok(amount) if amount >= 0 && trimmed.bytes().all(|b| b.is_ascii_digit()) => {
                    Ok(MaxAmount::Limit(amount))
                }
                _ => Err(Error::Invalid(s.to_string())),
            },
        }
    }
}

/// Parse a `--max-decimal-amount` value, checking its shape before the asset's
/// decimals are known.
pub fn parse_decimal_amount(s: &str) -> Result<String, Error> {
    let trimmed = s.trim();
    validate_decimal(trimmed)?;
    Ok(trimmed.to_string())
}

/// Enforce a cap in the asset's base units against a `required` amount in the
/// same units.
pub fn enforce(required: i128, cap: i128) -> Result<(), Error> {
    if required > cap {
        return Err(Error::Exceeded { required, cap });
    }
    Ok(())
}

/// Validate that `s` is a non-negative decimal number (no sign, no exponent),
/// without needing to know the asset's decimals yet.
fn validate_decimal(s: &str) -> Result<(), Error> {
    if s.is_empty() {
        return Err(Error::InvalidDecimal(s.to_string()));
    }
    let (int_part, frac_part) = match s.split_once('.') {
        Some((i, f)) => (i, f),
        None => (s, ""),
    };
    // A bare "." , "1." , or ".5" are all rejected: require digits on both sides
    // that are actually present (an empty fractional part from a trailing "." is
    // invalid).
    if int_part.is_empty() || !int_part.bytes().all(|b| b.is_ascii_digit()) {
        return Err(Error::InvalidDecimal(s.to_string()));
    }
    if s.contains('.') && (frac_part.is_empty() || !frac_part.bytes().all(|b| b.is_ascii_digit())) {
        return Err(Error::InvalidDecimal(s.to_string()));
    }
    Ok(())
}

/// Convert a non-negative decimal string in display units to the asset's
/// smallest unit, scaling by `10^decimals`. Rejects more fractional digits than
/// the asset supports rather than silently rounding.
pub fn decimal_to_raw(s: &str, decimals: u32) -> Result<i128, Error> {
    validate_decimal(s)?;
    if decimals > MAX_DECIMALS {
        return Err(Error::DecimalsTooLarge { decimals });
    }
    let (int_part, frac_part) = s.split_once('.').unwrap_or((s, ""));
    let scale = decimals as usize;
    if frac_part.len() > scale {
        return Err(Error::TooPrecise {
            amount: s.to_string(),
            decimals,
        });
    }
    // Right-pad the fractional digits to exactly `decimals`, then the whole
    // string is the integer value in the smallest unit.
    let mut digits = String::with_capacity(int_part.len() + scale);
    digits.push_str(int_part);
    digits.push_str(frac_part);
    for _ in 0..(scale - frac_part.len()) {
        digits.push('0');
    }
    // Strip leading zeros so `parse` doesn't choke on e.g. "0010000000"; an
    // all-zero string parses as 0.
    let digits = digits.trim_start_matches('0');
    if digits.is_empty() {
        return Ok(0);
    }
    digits
        .parse::<i128>()
        .map_err(|_| Error::Overflow(s.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disable_keywords_parse_to_unlimited() {
        for kw in ["none", "off", "unlimited", "NONE", "Off"] {
            assert_eq!(kw.parse::<MaxAmount>().unwrap(), MaxAmount::Unlimited);
        }
    }

    #[test]
    fn max_amount_is_in_base_units() {
        assert_eq!(
            "10000000".parse::<MaxAmount>().unwrap(),
            MaxAmount::Limit(10_000_000)
        );
        assert_eq!("0".parse::<MaxAmount>().unwrap(), MaxAmount::Limit(0));
    }

    #[test]
    fn invalid_max_amounts_are_rejected() {
        for bad in ["", "abc", "-1", "+1", "1.5", "1e3"] {
            let err = bad.parse::<MaxAmount>().unwrap_err();
            assert_eq!(err.error_type(), "invalid_max_amount", "`{bad}`");
        }
    }

    #[test]
    fn decimal_amounts_are_parsed() {
        assert_eq!(parse_decimal_amount("1").unwrap(), "1");
        assert_eq!(parse_decimal_amount("0.5").unwrap(), "0.5");
    }

    #[test]
    fn invalid_decimal_amounts_are_rejected() {
        for bad in ["", "abc", "-1", "1.", ".5", "1.2.3", "1e3", "none"] {
            let err = parse_decimal_amount(bad).unwrap_err();
            assert_eq!(err.error_type(), "invalid_max_decimal_amount", "`{bad}`");
        }
    }

    #[test]
    fn decimal_amounts_scale_to_base_units() {
        assert_eq!(decimal_to_raw("1", 7).unwrap(), 10_000_000);
        assert_eq!(decimal_to_raw("0.5", 7).unwrap(), 5_000_000);
        assert_eq!(decimal_to_raw("1.2345", 7).unwrap(), 12_345_000);
        assert_eq!(decimal_to_raw("0", 7).unwrap(), 0);
        assert_eq!(decimal_to_raw("10", 0).unwrap(), 10);
    }

    #[test]
    fn excess_precision_is_rejected() {
        let err = decimal_to_raw("1.12345678", 7).unwrap_err();
        assert_eq!(err.error_type(), "max_amount_too_precise");
    }

    #[test]
    fn oversized_decimals_are_rejected_before_scaling() {
        let err = decimal_to_raw("1", MAX_DECIMALS + 1).unwrap_err();
        assert_eq!(err.error_type(), "asset_decimals_too_large");
        let err = decimal_to_raw("1", u32::MAX).unwrap_err();
        assert_eq!(err.error_type(), "asset_decimals_too_large");
        assert!(decimal_to_raw("0", MAX_DECIMALS).is_ok());
    }

    #[test]
    fn enforce_passes_under_cap_and_fails_over() {
        assert!(enforce(10_000_000, 10_000_000).is_ok());
        assert!(enforce(9_999_999, 10_000_000).is_ok());
        let err = enforce(10_000_001, 10_000_000).unwrap_err();
        assert_eq!(err.error_type(), "max_amount_exceeded");
    }
}
