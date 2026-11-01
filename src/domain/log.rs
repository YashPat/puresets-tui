use std::collections::BTreeMap;

use super::{Exercise, Muscle, Sets, Week};

/// Sets credited to each muscle in one week. Muscles with no sets are absent.
pub type WeekSets = BTreeMap<Muscle, Sets>;

/// Direction of a manual adjustment on the home screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    Add(Sets),
    Remove(Sets),
}

/// Weekly set totals per muscle: everything the user has logged.
///
/// Only totals are kept, never individual workouts, like the iOS app.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TrainingLog {
    weeks: BTreeMap<Week, WeekSets>,
}

impl TrainingLog {
    pub fn new() -> Self {
        Self::default()
    }

    /// Rebuilds a log from stored weeks, dropping empty entries.
    pub fn from_weeks(weeks: BTreeMap<Week, WeekSets>) -> Self {
        let mut log = Self { weeks };
        for sets in log.weeks.values_mut() {
            sets.retain(|_, value| !value.is_zero());
        }
        log.weeks.retain(|_, sets| !sets.is_empty());
        log
    }

    pub fn weeks(&self) -> &BTreeMap<Week, WeekSets> {
        &self.weeks
    }

    pub fn sets(&self, week: Week, muscle: Muscle) -> Sets {
        self.weeks
            .get(&week)
            .and_then(|sets| sets.get(&muscle))
            .copied()
            .unwrap_or_default()
    }

    /// A copy of one week, for undo.
    pub fn week(&self, week: Week) -> WeekSets {
        self.weeks.get(&week).cloned().unwrap_or_default()
    }

    /// Puts back a week captured with [`TrainingLog::week`].
    pub fn restore(&mut self, week: Week, sets: WeekSets) {
        if sets.is_empty() {
            self.weeks.remove(&week);
        } else {
            self.weeks.insert(week, sets);
        }
    }

    /// Credits `count` sets of `exercise` to every muscle it works.
    pub fn log_exercise(&mut self, week: Week, exercise: &Exercise, count: u32) {
        for credit in exercise.credits(count) {
            self.adjust(week, credit.muscle, Change::Add(credit.sets));
        }
    }

    /// Adds or removes sets for one muscle. Totals never drop below zero.
    pub fn adjust(&mut self, week: Week, muscle: Muscle, change: Change) {
        let current = self.sets(week, muscle);
        let updated = match change {
            Change::Add(amount) => current + amount,
            Change::Remove(amount) => current.saturating_sub(amount),
        };
        if updated == current {
            return;
        }
        let sets = self.weeks.entry(week).or_default();
        if updated.is_zero() {
            sets.remove(&muscle);
            if sets.is_empty() {
                self.weeks.remove(&week);
            }
        } else {
            sets.insert(muscle, updated);
        }
    }

    /// All sets logged in `week`, across muscles.
    pub fn week_total(&self, week: Week) -> Sets {
        self.weeks
            .get(&week)
            .map(|sets| sets.values().copied().sum())
            .unwrap_or_default()
    }

    /// One muscle's sets summed over several weeks.
    pub fn total_for(&self, muscle: Muscle, weeks: &[Week]) -> Sets {
        weeks.iter().map(|&week| self.sets(week, muscle)).sum()
    }

    pub fn earliest_week(&self) -> Option<Week> {
        self.weeks.keys().next().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Catalog;

    fn week(n: i8) -> Week {
        Week::new(2026, n).unwrap()
    }

    #[test]
    fn logging_credits_every_worked_muscle() {
        let catalog = Catalog::built_in().unwrap();
        let mut log = TrainingLog::new();
        log.log_exercise(week(44), catalog.get("Bench Press").unwrap(), 3);
        log.log_exercise(week(44), catalog.get("Bench Press").unwrap(), 1);

        assert_eq!(log.sets(week(44), Muscle::Chest), Sets::whole(4));
        assert_eq!(log.sets(week(44), Muscle::Triceps), Sets::whole(2));
        assert_eq!(log.sets(week(43), Muscle::Chest), Sets::ZERO);
        assert_eq!(log.week_total(week(44)), Sets::whole(10));
    }

    #[test]
    fn removing_stops_at_zero_and_drops_empty_weeks() {
        let mut log = TrainingLog::new();
        log.adjust(week(44), Muscle::Abs, Change::Add(Sets::HALF));
        log.adjust(week(44), Muscle::Abs, Change::Remove(Sets::ONE));
        assert_eq!(log.sets(week(44), Muscle::Abs), Sets::ZERO);
        assert!(log.weeks().is_empty());
        assert_eq!(log.earliest_week(), None);
    }

    #[test]
    fn restore_undoes_a_change() {
        let mut log = TrainingLog::new();
        log.adjust(week(44), Muscle::Lats, Change::Add(Sets::whole(2)));
        let snapshot = log.week(week(44));
        log.adjust(week(44), Muscle::Lats, Change::Add(Sets::whole(5)));
        log.restore(week(44), snapshot);
        assert_eq!(log.sets(week(44), Muscle::Lats), Sets::whole(2));

        log.restore(week(44), WeekSets::new());
        assert!(log.weeks().is_empty());
    }

    #[test]
    fn totals_span_weeks() {
        let mut log = TrainingLog::new();
        log.adjust(week(42), Muscle::Quads, Change::Add(Sets::whole(6)));
        log.adjust(week(44), Muscle::Quads, Change::Add(Sets::whole(4)));
        let span: Vec<Week> = week(44).trailing(4).collect();
        assert_eq!(log.total_for(Muscle::Quads, &span), Sets::whole(10));
        assert_eq!(log.earliest_week(), Some(week(42)));
    }

    #[test]
    fn from_weeks_drops_zero_entries() {
        let mut weeks = BTreeMap::new();
        weeks.insert(week(1), BTreeMap::from([(Muscle::Abs, Sets::ZERO)]));
        assert!(TrainingLog::from_weeks(weeks).weeks().is_empty());
    }
}
