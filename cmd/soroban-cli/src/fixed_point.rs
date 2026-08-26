use std::fmt::{self, Display};

/// A fixed-point number: a raw integer `value` interpreted as scaled by
/// `10^decimals` (e.g. a token balance in its smallest unit alongside the
/// token's `decimals`).
///
/// Formatting inserts the decimal point by manipulating the digit string
/// directly, so it never computes `10.pow(decimals)`. That means it cannot
/// overflow regardless of how large `decimals` is — important because
/// `decimals` is often contract-controlled and unbounded.
#[derive(Clone, Copy, Debug)]
pub struct FixedPoint {
    value: i128,
    decimals: u32,
}

impl FixedPoint {
    #[must_use]
    pub fn new(value: i128, decimals: u32) -> Self {
        Self { value, decimals }
    }
}

impl Display for FixedPoint {
    /// Renders the value as a decimal string with trailing zeros trimmed
    /// (e.g. `12345000` at 7 decimals → `1.2345`, `10000000` → `1`).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.decimals == 0 {
            return write!(f, "{}", self.value);
        }

        let decimals = self.decimals as usize;
        // `unsigned_abs` yields the magnitude as a u128, which is safe even for
        // `i128::MIN` (whose absolute value doesn't fit in an i128).
        let digits = self.value.unsigned_abs().to_string();
        // Left-pad so there is at least one integer digit ahead of the
        // `decimals` fractional digits.
        let digits = if digits.len() <= decimals {
            format!("{digits:0>width$}", width = decimals + 1)
        } else {
            digits
        };

        let point = digits.len() - decimals;
        let integer_part = &digits[..point];
        let fractional_part = digits[point..].trim_end_matches('0');
        let sign = if self.value < 0 { "-" } else { "" };

        if fractional_part.is_empty() {
            write!(f, "{sign}{integer_part}")
        } else {
            write!(f, "{sign}{integer_part}.{fractional_part}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fmt(value: i128, decimals: u32) -> String {
        FixedPoint::new(value, decimals).to_string()
    }

    #[test]
    #[allow(clippy::unreadable_literal)]
    fn formats_typical_values() {
        assert_eq!(fmt(0, 7), "0");
        assert_eq!(fmt(1234567, 7), "0.1234567");
        assert_eq!(fmt(12345000, 7), "1.2345");
        assert_eq!(fmt(10000000, 7), "1");
        assert_eq!(fmt(123456789012345, 7), "12345678.9012345");
        assert_eq!(fmt(1, 7), "0.0000001");
        assert_eq!(fmt(12345, 0), "12345");
        assert_eq!(fmt(12345, 1), "1234.5");
    }

    #[test]
    #[allow(clippy::unreadable_literal)]
    fn formats_negative_values() {
        assert_eq!(fmt(-1234567, 7), "-0.1234567");
        assert_eq!(fmt(-12345000, 7), "-1.2345");
        assert_eq!(fmt(-10000000, 7), "-1");
        assert_eq!(fmt(-5, 0), "-5");
    }

    #[test]
    fn handles_i128_bounds_without_overflow() {
        assert_eq!(
            fmt(i128::MIN, 18),
            "-170141183460469231731.687303715884105728"
        );
        assert_eq!(
            fmt(i128::MAX, 18),
            "170141183460469231731.687303715884105727"
        );
    }

    #[test]
    fn large_decimals_do_not_overflow() {
        // `10.pow(decimals)` would overflow i128 for decimals >= 39 (and panic
        // with a zero divisor for >= 128); string placement handles any scale.
        assert_eq!(fmt(1, 39), "0.000000000000000000000000000000000000001");
        assert_eq!(fmt(123, 255).chars().take(2).collect::<String>(), "0.");
    }
}
