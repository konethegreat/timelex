// Written by Kone Tshivhinda & Claude
//! Rand amounts as integer micro-rand.
//!
//! A model call's cost is a few thousandths of a rand, so floating point would make two equal
//! runs spend slightly different amounts and a cap test flaky. One micro-rand (R0.000001) is small
//! enough for every published rate and exact in a `u64`.

use std::fmt;

/// A money amount in micro-rand (1 ZAR = 1 000 000 micro-rand).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Rand(u64);

impl Rand {
    /// R0.00.
    pub const ZERO: Self = Self(0);

    /// An amount from micro-rand.
    pub const fn from_micros(micros: u64) -> Self {
        Self(micros)
    }

    /// The amount in micro-rand.
    pub const fn micros(self) -> u64 {
        self.0
    }

    /// Add, saturating at the `u64` ceiling. A run that could overflow has far larger problems
    /// than an inexact total, and saturation keeps it from wrapping to a smaller spend.
    pub fn saturating_add(self, other: Self) -> Self {
        Self(self.0.saturating_add(other.0))
    }
}

impl fmt::Display for Rand {
    /// `R0.007200`: the whole rand, a dot, and six micro-rand digits.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "R{}.{:06}", self.0 / 1_000_000, self.0 % 1_000_000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_keeps_six_places() {
        assert_eq!(Rand::ZERO.to_string(), "R0.000000");
        assert_eq!(Rand::from_micros(7_200).to_string(), "R0.007200");
        assert_eq!(Rand::from_micros(1_500_000).to_string(), "R1.500000");
    }

    #[test]
    fn addition_saturates() {
        let nearly = Rand::from_micros(u64::MAX - 1);
        assert_eq!(nearly.saturating_add(Rand::from_micros(10)).micros(), u64::MAX);
    }
}
