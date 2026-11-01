use std::{fmt, str::FromStr};

use jiff::civil::{Date, ISOWeekDate, Weekday};
use serde::{Deserialize, Serialize};

/// An ISO 8601 week (Monday to Sunday), the unit all volume is tracked in.
///
/// Matches the iOS app's `Week`, which uses Monday-first weeks with ISO
/// numbering. Serializes as `"2026-W44"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Week {
    year: i16,
    number: i8,
}

impl Week {
    /// The week containing `date`.
    pub fn containing(date: Date) -> Self {
        let iso = date.iso_week_date();
        Self {
            year: iso.year(),
            number: iso.week(),
        }
    }

    /// # Errors
    ///
    /// Returns an error if `number` is not a week of `year` (for example W53
    /// of a 52-week year).
    pub fn new(year: i16, number: i8) -> Result<Self, ParseWeekError> {
        ISOWeekDate::new(year, number, Weekday::Monday)
            .map(|_| Self { year, number })
            .map_err(|_| ParseWeekError(format!("{year}-W{number:02}")))
    }

    pub const fn year(self) -> i16 {
        self.year
    }

    pub const fn number(self) -> i8 {
        self.number
    }

    pub fn monday(self) -> Date {
        self.day(Weekday::Monday)
    }

    pub fn sunday(self) -> Date {
        self.day(Weekday::Sunday)
    }

    fn day(self, weekday: Weekday) -> Date {
        ISOWeekDate::new(self.year, self.number, weekday)
            .expect("Week is only constructed from valid ISO weeks")
            .date()
    }

    #[must_use]
    pub fn previous(self) -> Self {
        Self::containing(
            self.monday()
                .yesterday()
                .expect("weeks stay far from Date::MIN"),
        )
    }

    #[must_use]
    pub fn next(self) -> Self {
        Self::containing(
            self.sunday()
                .tomorrow()
                .expect("weeks stay far from Date::MAX"),
        )
    }

    /// This week and the `count - 1` weeks before it, newest first.
    pub fn trailing(self, count: usize) -> impl Iterator<Item = Self> {
        std::iter::successors(Some(self), |week| Some(week.previous())).take(count)
    }

    /// Date range for display, e.g. `Oct 26 – Nov 1`.
    pub fn range_label(self) -> String {
        format!(
            "{} – {}",
            self.monday().strftime("%b %-d"),
            self.sunday().strftime("%b %-d")
        )
    }
}

impl fmt::Display for Week {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-W{:02}", self.year, self.number)
    }
}

impl FromStr for Week {
    type Err = ParseWeekError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let invalid = || ParseWeekError(s.to_owned());
        let (year, number) = s.split_once("-W").ok_or_else(invalid)?;
        let year: i16 = year.parse().map_err(|_| invalid())?;
        let number: i8 = number.parse().map_err(|_| invalid())?;
        Self::new(year, number).map_err(|_| invalid())
    }
}

impl TryFrom<String> for Week {
    type Error = ParseWeekError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl From<Week> for String {
    fn from(week: Week) -> Self {
        week.to_string()
    }
}

/// A string that is not a valid `YYYY-Www` ISO week.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseWeekError(String);

impl fmt::Display for ParseWeekError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid ISO week {:?}: expected YYYY-Www", self.0)
    }
}

impl std::error::Error for ParseWeekError {}

#[cfg(test)]
mod tests {
    use jiff::civil::date;

    use super::*;

    #[test]
    fn weeks_start_on_monday() {
        let week = Week::containing(date(2026, 11, 1)); // a Sunday
        assert_eq!(week.to_string(), "2026-W44");
        assert_eq!(week.monday(), date(2026, 10, 26));
        assert_eq!(week.sunday(), date(2026, 11, 1));
        assert_eq!(week.range_label(), "Oct 26 – Nov 1");
    }

    #[test]
    fn steps_across_year_boundaries() {
        // 2026 has 53 ISO weeks; 2027-W01 starts on Monday 2027-01-04.
        let last = Week::new(2026, 53).unwrap();
        assert_eq!(last.next(), Week::new(2027, 1).unwrap());
        assert_eq!(Week::new(2027, 1).unwrap().previous(), last);
        assert_eq!(Week::containing(date(2027, 1, 1)), last);
    }

    #[test]
    fn trailing_lists_newest_first() {
        let week = Week::new(2026, 2).unwrap();
        let weeks: Vec<String> = week.trailing(3).map(|w| w.to_string()).collect();
        assert_eq!(weeks, ["2026-W02", "2026-W01", "2025-W52"]);
    }

    #[test]
    fn parses_and_rejects() {
        assert_eq!("2026-W07".parse(), Week::new(2026, 7));
        for bad in ["2026-07", "2026-W00", "2025-W53", "W12", ""] {
            assert!(bad.parse::<Week>().is_err(), "{bad:?} should be rejected");
        }
    }

    #[test]
    fn orders_chronologically() {
        assert!(Week::new(2025, 52).unwrap() < Week::new(2026, 1).unwrap());
    }
}
