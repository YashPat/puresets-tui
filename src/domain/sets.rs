use std::{fmt, iter::Sum, ops::Add};

use serde::{Deserialize, Serialize};

/// A non-negative number of sets, counted in half-set steps.
///
/// Secondary muscles earn half a set per logged set, so every quantity the
/// app stores is a multiple of 0.5. Keeping the count in halves makes the
/// arithmetic exact. It serializes as a plain JSON number (`9.5`).
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(try_from = "f64", into = "f64")]
pub struct Sets {
    halves: u32,
}

impl Sets {
    pub const ZERO: Self = Self { halves: 0 };
    pub const HALF: Self = Self { halves: 1 };
    pub const ONE: Self = Self { halves: 2 };

    pub const fn whole(count: u32) -> Self {
        Self {
            halves: count.saturating_mul(2),
        }
    }

    pub const fn from_halves(halves: u32) -> Self {
        Self { halves }
    }

    pub const fn halves(self) -> u32 {
        self.halves
    }

    pub const fn is_zero(self) -> bool {
        self.halves == 0
    }

    pub fn as_f64(self) -> f64 {
        f64::from(self.halves) / 2.0
    }

    #[must_use]
    pub const fn saturating_sub(self, other: Self) -> Self {
        Self {
            halves: self.halves.saturating_sub(other.halves),
        }
    }

    #[must_use]
    pub const fn saturating_mul(self, factor: u32) -> Self {
        Self {
            halves: self.halves.saturating_mul(factor),
        }
    }
}

impl Add for Sets {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self {
            halves: self.halves.saturating_add(other.halves),
        }
    }
}

impl Sum for Sets {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self::ZERO, Add::add)
    }
}

/// Formats like the iOS app: `9`, `9.5`. Honours width and alignment flags.
impl fmt::Display for Sets {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let whole = self.halves / 2;
        let text = if self.halves.is_multiple_of(2) {
            whole.to_string()
        } else {
            format!("{whole}.5")
        };
        f.pad(&text)
    }
}

impl From<Sets> for f64 {
    fn from(sets: Sets) -> Self {
        sets.as_f64()
    }
}

impl TryFrom<f64> for Sets {
    type Error = InvalidSetsError;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        let doubled = value * 2.0;
        let valid = doubled.is_finite()
            && doubled >= 0.0
            && doubled.fract() <= 0.0
            && doubled <= f64::from(u32::MAX);
        if !valid {
            return Err(InvalidSetsError(value));
        }
        #[expect(
            clippy::as_conversions,
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "checked above: finite, non-negative, integral and within u32"
        )]
        let halves = doubled as u32;
        Ok(Self { halves })
    }
}

/// A stored set count that is negative, not finite, or not a multiple of 0.5.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InvalidSetsError(f64);

impl fmt::Display for InvalidSetsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid set count {}: expected a non-negative multiple of 0.5",
            self.0
        )
    }
}

impl std::error::Error for InvalidSetsError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn displays_whole_and_half_sets() {
        assert_eq!(Sets::whole(9).to_string(), "9");
        assert_eq!(Sets::from_halves(19).to_string(), "9.5");
        assert_eq!(Sets::ZERO.to_string(), "0");
        assert_eq!(Sets::HALF.to_string(), "0.5");
    }

    #[test]
    fn subtraction_stops_at_zero() {
        assert_eq!(Sets::HALF.saturating_sub(Sets::ONE), Sets::ZERO);
    }

    #[test]
    fn converts_from_valid_floats_only() {
        assert_eq!(Sets::try_from(3.5), Ok(Sets::from_halves(7)));
        assert_eq!(Sets::try_from(0.0), Ok(Sets::ZERO));
        for bad in [-1.0, 0.25, f64::NAN, f64::INFINITY] {
            assert!(Sets::try_from(bad).is_err(), "{bad} should be rejected");
        }
    }

    #[test]
    fn serde_round_trips_as_a_number() {
        let json = serde_json::to_string(&Sets::from_halves(5)).unwrap();
        assert_eq!(json, "2.5");
        let back: Sets = serde_json::from_str(&json).unwrap();
        assert_eq!(back, Sets::from_halves(5));
        assert!(serde_json::from_str::<Sets>("1.3").is_err());
    }
}
