// Written by Kone Tshivhinda & Claude
//! Billing-unit and fee maths.
//!
//! Pure functions, no I/O, no floating point. Money is `i64` **cents** (ZAR minor units);
//! durations are whole **seconds**. Rounding of money is half-away-from-zero, matching
//! ClearCut's convention (ClearCut ADR-0002), so a pro-forma built here reconciles to the cent
//! with an invoice calculated there.
//!
//! ```
//! use timelex_units::{billable_units, value_cents, Rounding, UnitPolicy};
//!
//! // 47 minutes, 6-minute units, round up, R2 400.00 per hour.
//! let policy = UnitPolicy { unit_minutes: 6, rounding: Rounding::Up, minimum_units: 1 };
//! let units = billable_units(47 * 60, &policy).unwrap();
//! assert_eq!(units, 8); // 48 minutes
//! assert_eq!(value_cents(units, policy.unit_minutes, 240_000).unwrap(), 192_000); // R1 920.00
//! ```

use std::fmt;

/// How a duration that is not an exact multiple of the unit is rounded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rounding {
    /// Always round up to the next whole unit (common in fee agreements).
    Up,
    /// Always round down.
    Down,
    /// Round to the nearest unit; an exact half rounds up.
    Nearest,
}

/// A firm's or client's rule for turning time into billing units.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnitPolicy {
    /// Length of one unit in minutes. Valid range 1..=60 (commonly 5, 6, 10 or 15).
    pub unit_minutes: u32,
    /// Rounding mode for partial units.
    pub rounding: Rounding,
    /// Minimum units charged for any non-zero duration (0 allows rounding down to nothing).
    pub minimum_units: u32,
}

impl Default for UnitPolicy {
    /// Six-minute units (0.1 h), round up, one-unit minimum.
    fn default() -> Self {
        Self {
            unit_minutes: 6,
            rounding: Rounding::Up,
            minimum_units: 1,
        }
    }
}

/// Errors from unit and value calculations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitsError {
    /// `unit_minutes` was outside 1..=60.
    InvalidUnitMinutes(u32),
    /// A rate was negative.
    NegativeRate,
    /// The result does not fit in the target integer type.
    Overflow,
}

impl fmt::Display for UnitsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UnitsError::InvalidUnitMinutes(m) => write!(f, "unit_minutes must be 1..=60, got {m}"),
            UnitsError::NegativeRate => write!(f, "hourly rate must not be negative"),
            UnitsError::Overflow => write!(f, "arithmetic overflow"),
        }
    }
}

impl std::error::Error for UnitsError {}

fn check_unit_minutes(unit_minutes: u32) -> Result<(), UnitsError> {
    if (1..=60).contains(&unit_minutes) {
        Ok(())
    } else {
        Err(UnitsError::InvalidUnitMinutes(unit_minutes))
    }
}

/// Convert a duration in seconds to billing units under `policy`.
///
/// A duration of zero seconds is always zero units. Any other duration is charged at least
/// `policy.minimum_units`.
pub fn billable_units(seconds: u64, policy: &UnitPolicy) -> Result<u64, UnitsError> {
    check_unit_minutes(policy.unit_minutes)?;
    if seconds == 0 {
        return Ok(0);
    }
    let unit_secs = u64::from(policy.unit_minutes) * 60;
    let whole = seconds / unit_secs;
    let rem = seconds % unit_secs;
    let units = match policy.rounding {
        Rounding::Down => whole,
        Rounding::Up => whole + u64::from(rem > 0),
        // Exact half (rem * 2 == unit_secs) rounds up.
        Rounding::Nearest => whole + u64::from(rem * 2 >= unit_secs),
    };
    Ok(units.max(u64::from(policy.minimum_units)))
}

/// Value in cents of `units` billing units at `hourly_rate_cents` per hour.
///
/// `value = units * unit_minutes * rate / 60`, rounded half away from zero, using 128-bit
/// intermediates so large inputs cannot silently wrap.
pub fn value_cents(units: u64, unit_minutes: u32, hourly_rate_cents: i64) -> Result<i64, UnitsError> {
    check_unit_minutes(unit_minutes)?;
    if hourly_rate_cents < 0 {
        return Err(UnitsError::NegativeRate);
    }
    let numerator = i128::from(units)
        .checked_mul(i128::from(unit_minutes))
        .and_then(|v| v.checked_mul(i128::from(hourly_rate_cents)))
        .ok_or(UnitsError::Overflow)?;
    // Non-negative numerator, so half-away-from-zero is "add half, floor".
    let rounded = (numerator + 30) / 60;
    i64::try_from(rounded).map_err(|_| UnitsError::Overflow)
}

/// Convenience: seconds straight to (units, cents).
pub fn price_duration(
    seconds: u64,
    policy: &UnitPolicy,
    hourly_rate_cents: i64,
) -> Result<(u64, i64), UnitsError> {
    let units = billable_units(seconds, policy)?;
    let cents = value_cents(units, policy.unit_minutes, hourly_rate_cents)?;
    Ok((units, cents))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pol(rounding: Rounding, minimum_units: u32) -> UnitPolicy {
        UnitPolicy {
            unit_minutes: 6,
            rounding,
            minimum_units,
        }
    }

    #[test]
    fn readme_example_47_minutes_round_up() {
        let (units, cents) = price_duration(47 * 60, &UnitPolicy::default(), 240_000).unwrap();
        assert_eq!(units, 8);
        assert_eq!(cents, 192_000); // R1 920.00
    }

    #[test]
    fn zero_seconds_is_zero_units_even_with_minimum() {
        assert_eq!(billable_units(0, &pol(Rounding::Up, 3)).unwrap(), 0);
    }

    #[test]
    fn exact_multiples_do_not_round_up() {
        assert_eq!(billable_units(6 * 60, &pol(Rounding::Up, 1)).unwrap(), 1);
        assert_eq!(billable_units(12 * 60, &pol(Rounding::Up, 1)).unwrap(), 2);
    }

    #[test]
    fn one_second_over_rounds_up_to_next_unit() {
        assert_eq!(billable_units(6 * 60 + 1, &pol(Rounding::Up, 1)).unwrap(), 2);
    }

    #[test]
    fn down_rounding_and_minimum_unit() {
        // 5 minutes rounds down to 0 units, then the minimum applies.
        assert_eq!(billable_units(5 * 60, &pol(Rounding::Down, 0)).unwrap(), 0);
        assert_eq!(billable_units(5 * 60, &pol(Rounding::Down, 1)).unwrap(), 1);
        assert_eq!(billable_units(11 * 60, &pol(Rounding::Down, 1)).unwrap(), 1);
    }

    #[test]
    fn nearest_rounds_exact_half_up() {
        let p = pol(Rounding::Nearest, 0);
        assert_eq!(billable_units(2 * 60 + 59, &p).unwrap(), 0); // 2:59 of 6:00 -> below half
        assert_eq!(billable_units(3 * 60, &p).unwrap(), 1); // exactly half -> up
        assert_eq!(billable_units(9 * 60, &p).unwrap(), 2); // 1.5 units -> 2
        assert_eq!(billable_units(8 * 60 + 59, &p).unwrap(), 1); // just under 1.5 -> 1
    }

    #[test]
    fn units_never_decrease_as_time_grows() {
        for rounding in [Rounding::Up, Rounding::Down, Rounding::Nearest] {
            let p = pol(rounding, 1);
            let mut prev = 0;
            for secs in 0..=3 * 3600 {
                let u = billable_units(secs, &p).unwrap();
                assert!(u >= prev, "{rounding:?}: units fell at {secs}s");
                prev = u;
            }
        }
    }

    #[test]
    fn value_rounds_half_away_from_zero() {
        // 1 unit of 6 min at 12345 c/h = 1234.5 c -> 1235
        assert_eq!(value_cents(1, 6, 12_345).unwrap(), 1235);
        // 1 unit of 6 min at 12344 c/h = 1234.4 c -> 1234
        assert_eq!(value_cents(1, 6, 12_344).unwrap(), 1234);
    }

    #[test]
    fn value_is_exact_when_divisible() {
        // 10 units of 6 minutes = 1 hour exactly.
        assert_eq!(value_cents(10, 6, 150_000).unwrap(), 150_000);
        // 15-minute units: 4 units = 1 hour.
        assert_eq!(value_cents(4, 15, 99_999).unwrap(), 99_999);
    }

    #[test]
    fn fifteen_minute_policy_example() {
        let p = UnitPolicy {
            unit_minutes: 15,
            rounding: Rounding::Up,
            minimum_units: 1,
        };
        let (units, cents) = price_duration(16 * 60, &p, 200_000).unwrap();
        assert_eq!(units, 2);
        assert_eq!(cents, 100_000); // 30 minutes at R2 000/h
    }

    #[test]
    fn invalid_unit_minutes_rejected() {
        let bad = UnitPolicy {
            unit_minutes: 0,
            ..UnitPolicy::default()
        };
        assert_eq!(billable_units(60, &bad), Err(UnitsError::InvalidUnitMinutes(0)));
        assert_eq!(value_cents(1, 61, 100), Err(UnitsError::InvalidUnitMinutes(61)));
    }

    #[test]
    fn negative_rate_rejected() {
        assert_eq!(value_cents(1, 6, -1), Err(UnitsError::NegativeRate));
    }

    #[test]
    fn overflow_is_reported_not_wrapped() {
        assert_eq!(value_cents(u64::MAX, 60, i64::MAX), Err(UnitsError::Overflow));
    }

    #[test]
    fn zero_rate_is_zero_value() {
        assert_eq!(value_cents(50, 6, 0).unwrap(), 0);
    }
}
