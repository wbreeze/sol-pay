//! Human amounts to mint base units and back.
//!
//! Every amount this crate takes is in base units, and USDC has six decimals.
//! An integrator who scales twice turns an intended 50 USDC into 50,000,000
//! of allowance, and nothing rejects it: `approve` checks no balance, and the
//! program's delegate check only compares the allowance against the limit. The
//! payer's chosen cap silently becomes their whole balance.
//!
//! Owning the conversion removes that error class. Validating its output could
//! not, because no validator knows what the payer meant.

/// Decimal strings, not floats. `0.1` is not representable in binary floating
/// point, and a payment library that rounds is not one anybody can audit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitsError {
    Empty,
    NotANumber,
    /// More digits after the point than the mint has decimals, so the value
    /// cannot be represented without discarding some of what was asked for.
    TooPrecise { decimals: u8, given: usize },
    /// The amount does not fit in a u64 at this mint's scale.
    Overflow,
}

impl core::fmt::Display for UnitsError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            UnitsError::Empty => write!(f, "amount is empty"),
            UnitsError::NotANumber => write!(f, "amount is not a decimal number"),
            UnitsError::TooPrecise { decimals, given } => write!(
                f,
                "amount has {given} decimal places, the mint has {decimals}"
            ),
            UnitsError::Overflow => write!(f, "amount does not fit in u64 base units"),
        }
    }
}

/// Parse a decimal amount into base units at a mint's scale.
///
/// Accepts `"12"`, `"12.5"`, `"0.000001"`, `".5"`, `"12."`. Rejects anything
/// with a sign, an exponent, separators, or more precision than the mint can
/// hold -- rounding someone's money down is not this function's business.
pub fn to_base_units(amount: &str, decimals: u8) -> Result<u64, UnitsError> {
    let amount = amount.trim();
    if amount.is_empty() {
        return Err(UnitsError::Empty);
    }

    let (whole, frac) = match amount.split_once('.') {
        Some((w, f)) => {
            if f.contains('.') {
                return Err(UnitsError::NotANumber);
            }
            (w, f)
        }
        None => (amount, ""),
    };
    if whole.is_empty() && frac.is_empty() {
        return Err(UnitsError::NotANumber);
    }
    if !whole.bytes().all(|b| b.is_ascii_digit()) || !frac.bytes().all(|b| b.is_ascii_digit()) {
        return Err(UnitsError::NotANumber);
    }

    let decimals = decimals as usize;
    if frac.len() > decimals {
        return Err(UnitsError::TooPrecise {
            decimals: decimals as u8,
            given: frac.len(),
        });
    }

    let mut units: u64 = 0;
    for b in whole.bytes().chain(frac.bytes()) {
        units = units
            .checked_mul(10)
            .and_then(|u| u.checked_add((b - b'0') as u64))
            .ok_or(UnitsError::Overflow)?;
    }
    // Pad the fraction out to the mint's scale.
    for _ in 0..(decimals - frac.len()) {
        units = units.checked_mul(10).ok_or(UnitsError::Overflow)?;
    }
    Ok(units)
}

/// Render base units as a decimal string, without trailing zeros.
///
/// `from_base_units(1_500_000, 6)` is `"1.5"`, not `"1.500000"`, and
/// `from_base_units(1_000_000, 6)` is `"1"`. Every output round-trips back
/// through [`to_base_units`] at the same scale.
pub fn from_base_units(units: u64, decimals: u8) -> String {
    let decimals = decimals as usize;
    if decimals == 0 {
        return units.to_string();
    }
    let scale = 10u64.pow(decimals as u32);
    let whole = units / scale;
    let frac = units % scale;
    if frac == 0 {
        return whole.to_string();
    }
    let frac = format!("{frac:0width$}", width = decimals);
    format!("{whole}.{}", frac.trim_end_matches('0'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_at_usdc_scale() {
        assert_eq!(to_base_units("1", 6).unwrap(), 1_000_000);
        assert_eq!(to_base_units("1.5", 6).unwrap(), 1_500_000);
        assert_eq!(to_base_units("0.000001", 6).unwrap(), 1);
        assert_eq!(to_base_units(".5", 6).unwrap(), 500_000);
        assert_eq!(to_base_units("12.", 6).unwrap(), 12_000_000);
        assert_eq!(to_base_units("0", 6).unwrap(), 0);
        assert_eq!(to_base_units("  2.25  ", 6).unwrap(), 2_250_000);
    }

    #[test]
    fn refuses_what_it_cannot_represent_rather_than_rounding() {
        assert_eq!(
            to_base_units("0.0000001", 6),
            Err(UnitsError::TooPrecise {
                decimals: 6,
                given: 7
            })
        );
        assert_eq!(to_base_units("", 6), Err(UnitsError::Empty));
        assert_eq!(to_base_units(".", 6), Err(UnitsError::NotANumber));
        assert_eq!(to_base_units("-1", 6), Err(UnitsError::NotANumber));
        assert_eq!(to_base_units("1e6", 6), Err(UnitsError::NotANumber));
        assert_eq!(to_base_units("1_000", 6), Err(UnitsError::NotANumber));
        assert_eq!(to_base_units("1.2.3", 6), Err(UnitsError::NotANumber));
        assert_eq!(to_base_units("184467440738", 9), Err(UnitsError::Overflow));
    }

    #[test]
    fn renders_without_trailing_zeros() {
        assert_eq!(from_base_units(1_500_000, 6), "1.5");
        assert_eq!(from_base_units(1_000_000, 6), "1");
        assert_eq!(from_base_units(1, 6), "0.000001");
        assert_eq!(from_base_units(0, 6), "0");
        assert_eq!(from_base_units(42, 0), "42");
    }

    #[test]
    fn round_trips() {
        for units in [0u64, 1, 999_999, 1_000_000, 1_500_000, u64::MAX / 2] {
            for decimals in [0u8, 2, 6, 9] {
                let text = from_base_units(units, decimals);
                assert_eq!(
                    to_base_units(&text, decimals).unwrap(),
                    units,
                    "{text} at {decimals} decimals"
                );
            }
        }
    }

    /// The fault this module exists to prevent.
    #[test]
    fn scaling_twice_is_not_silently_accepted() {
        let once = to_base_units("50", 6).unwrap();
        assert_eq!(once, 50_000_000);
        // Feeding the already-scaled number back in is the bug. It is a
        // different number, loudly, rather than the same one quietly.
        let twice = to_base_units(&once.to_string(), 6).unwrap();
        assert_ne!(twice, once);
        assert_eq!(twice, 50_000_000_000_000);
    }
}
