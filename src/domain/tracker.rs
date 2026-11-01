use super::{Catalog, Credit, Exercise, Target, TrainingLog, Week};

/// How many recently logged exercises are offered first in the log sheet.
pub const RECENT_LIMIT: usize = 8;

/// Everything the user owns: exercises, logged sets and preferences.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tracker {
    pub catalog: Catalog,
    pub log: TrainingLog,
    /// `None` until the user picks a goal on first launch.
    pub target: Option<Target>,
    /// The week the user started tracking; the home screen browses back to it.
    pub first_week: Option<Week>,
    recents: Vec<String>,
}

impl Tracker {
    pub fn new(catalog: Catalog) -> Self {
        Self {
            catalog,
            log: TrainingLog::new(),
            target: None,
            first_week: None,
            recents: Vec::new(),
        }
    }

    /// The target in effect, falling back to the default before onboarding.
    pub fn target_or_default(&self) -> Target {
        self.target.unwrap_or_default()
    }

    /// Logs `count` sets of the named exercise, returning what each muscle earned.
    ///
    /// Returns `None` if the exercise does not exist.
    pub fn log_sets(&mut self, week: Week, name: &str, count: u32) -> Option<Vec<Credit>> {
        let exercise = self.catalog.get(name)?;
        let credits = exercise.credits(count);
        let canonical = exercise.name().to_owned();
        self.log.log_exercise(week, exercise, count);
        self.remember(canonical);
        Some(credits)
    }

    /// Recently logged exercises that still exist, newest first.
    pub fn recents(&self) -> impl Iterator<Item = &Exercise> {
        self.recents
            .iter()
            .filter_map(|name| self.catalog.get(name))
    }

    pub fn recent_names(&self) -> &[String] {
        &self.recents
    }

    /// Restores the recent list from storage, keeping known names only.
    pub fn set_recents(&mut self, names: Vec<String>) {
        self.recents.clear();
        for name in names.into_iter().rev() {
            if self.catalog.get(&name).is_some() {
                self.remember(name);
            }
        }
    }

    pub fn forget_recent(&mut self, name: &str) {
        self.recents
            .retain(|recent| !recent.eq_ignore_ascii_case(name));
    }

    fn remember(&mut self, name: String) {
        self.forget_recent(&name);
        self.recents.insert(0, name);
        self.recents.truncate(RECENT_LIMIT);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Muscle, Sets};

    #[test]
    fn logging_updates_recents() {
        let mut tracker = Tracker::new(Catalog::built_in().unwrap());
        let week = Week::new(2026, 44).unwrap();
        tracker.log_sets(week, "squat", 2).unwrap();
        tracker.log_sets(week, "bench press", 1).unwrap();
        tracker.log_sets(week, "Squat", 1).unwrap();

        let recents: Vec<&str> = tracker.recents().map(Exercise::name).collect();
        assert_eq!(recents, ["Squat", "Bench Press"]);
        assert_eq!(tracker.log.sets(week, Muscle::Quads), Sets::whole(3));
        assert!(tracker.log_sets(week, "Nope", 1).is_none());
    }

    #[test]
    fn recents_are_capped() {
        let mut tracker = Tracker::new(Catalog::built_in().unwrap());
        let week = Week::new(2026, 44).unwrap();
        let names: Vec<String> = tracker
            .catalog
            .iter()
            .map(|e| e.name().to_owned())
            .collect();
        for name in &names {
            tracker.log_sets(week, name, 1);
        }
        assert_eq!(tracker.recents().count(), RECENT_LIMIT);
    }

    #[test]
    fn stored_recents_skip_unknown_names() {
        let mut tracker = Tracker::new(Catalog::built_in().unwrap());
        tracker.set_recents(vec!["Squat".into(), "Gone".into(), "Deadlift".into()]);
        assert_eq!(tracker.recent_names(), ["Squat", "Deadlift"]);
    }
}
