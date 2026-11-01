//! Reading and writing the user's data file.
//!
//! The file is plain JSON so it is easy to back up, inspect or edit by hand:
//!
//! ```json
//! {
//!   "version": 1,
//!   "targetSets": 8,
//!   "firstWeek": "2026-W44",
//!   "customExercises": [{ "name": "Cable Fly", "primaryMuscles": ["chest"] }],
//!   "notes": { "Squat": "Belt from 140" },
//!   "recentExercises": ["Squat"],
//!   "weeklySets": { "2026-W44": { "chest": 3, "triceps": 1.5 } }
//! }
//! ```

use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
};

use color_eyre::{
    Result,
    eyre::{WrapErr, bail, eyre},
};
use serde::{Deserialize, Serialize};

use crate::domain::{
    Catalog, ExerciseDraft, Muscle, Origin, Target, Tracker, TrainingLog, Week, WeekSets,
};

const FORMAT_VERSION: u32 = 1;

/// Environment variable that overrides the data file location.
pub const DATA_ENV: &str = "PURESETS_DATA";

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredData {
    version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    target_sets: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    first_week: Option<Week>,
    #[serde(default)]
    custom_exercises: Vec<StoredExercise>,
    /// Notes on built-in exercises, by name. Custom exercises keep theirs inline.
    #[serde(default)]
    notes: BTreeMap<String, String>,
    #[serde(default)]
    recent_exercises: Vec<String>,
    #[serde(default)]
    weekly_sets: BTreeMap<Week, WeekSets>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredExercise {
    name: String,
    primary_muscles: Vec<Muscle>,
    #[serde(default)]
    secondary_muscles: Vec<Muscle>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    note: String,
}

/// Where the data file lives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Storage {
    path: PathBuf,
}

/// A tracker read from disk, plus anything that had to be skipped.
#[derive(Debug)]
pub struct Loaded {
    pub tracker: Tracker,
    pub warnings: Vec<String>,
}

impl Storage {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// The location from `PURESETS_DATA`, or the platform data directory.
    ///
    /// # Errors
    ///
    /// Fails if the platform has no data directory and no override is set.
    pub fn default_location() -> Result<Self> {
        if let Some(path) = env::var_os(DATA_ENV).filter(|value| !value.is_empty()) {
            return Ok(Self::new(PathBuf::from(path)));
        }
        let base = dirs::data_dir()
            .ok_or_else(|| eyre!("no data directory on this platform; set {DATA_ENV}"))?;
        Ok(Self::new(base.join("puresets").join("data.json")))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Reads the data file, or starts fresh if there is none yet.
    ///
    /// # Errors
    ///
    /// Fails if the file exists but cannot be read or parsed. The caller
    /// should stop rather than overwrite it.
    pub fn load(&self, catalog: Catalog) -> Result<Loaded> {
        let json = match fs::read_to_string(&self.path) {
            Ok(json) => json,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Loaded {
                    tracker: Tracker::new(catalog),
                    warnings: Vec::new(),
                });
            }
            Err(err) => {
                return Err(err).wrap_err_with(|| format!("reading {}", self.path.display()));
            }
        };
        let stored: StoredData = serde_json::from_str(&json)
            .wrap_err_with(|| format!("parsing {}", self.path.display()))?;
        if stored.version > FORMAT_VERSION {
            bail!(
                "{} was written by a newer PureSets (format {}); this build reads format {FORMAT_VERSION}",
                self.path.display(),
                stored.version
            );
        }
        Ok(restore(stored, catalog))
    }

    /// Writes the data file atomically: a temporary file, then a rename.
    ///
    /// # Errors
    ///
    /// Fails if the directory cannot be created or the file cannot be written.
    pub fn save(&self, tracker: &Tracker) -> Result<()> {
        if let Some(dir) = self.path.parent().filter(|dir| !dir.as_os_str().is_empty()) {
            fs::create_dir_all(dir).wrap_err_with(|| format!("creating {}", dir.display()))?;
        }
        let mut json = serde_json::to_string_pretty(&snapshot(tracker))?;
        json.push('\n');
        let temp = self.path.with_extension("json.tmp");
        fs::write(&temp, json).wrap_err_with(|| format!("writing {}", temp.display()))?;
        fs::rename(&temp, &self.path).wrap_err_with(|| format!("replacing {}", self.path.display()))
    }
}

fn snapshot(tracker: &Tracker) -> StoredData {
    let mut custom_exercises = Vec::new();
    let mut notes = BTreeMap::new();
    for exercise in &tracker.catalog {
        match exercise.origin() {
            Origin::Custom => custom_exercises.push(StoredExercise {
                name: exercise.name().to_owned(),
                primary_muscles: exercise.primary().to_vec(),
                secondary_muscles: exercise.secondary().to_vec(),
                note: exercise.note().to_owned(),
            }),
            Origin::BuiltIn if !exercise.note().is_empty() => {
                notes.insert(exercise.name().to_owned(), exercise.note().to_owned());
            }
            Origin::BuiltIn => {}
        }
    }
    StoredData {
        version: FORMAT_VERSION,
        target_sets: tracker.target.map(Target::get),
        first_week: tracker.first_week,
        custom_exercises,
        notes,
        recent_exercises: tracker.recent_names().to_vec(),
        weekly_sets: tracker.log.weeks().clone(),
    }
}

fn restore(stored: StoredData, catalog: Catalog) -> Loaded {
    let mut warnings = Vec::new();
    let mut tracker = Tracker::new(catalog);

    tracker.target = stored.target_sets.and_then(|sets| match Target::new(sets) {
        Ok(target) => Some(target),
        Err(sets) => {
            warnings.push(format!("Ignored target of {sets} sets; pick a new one."));
            None
        }
    });
    tracker.first_week = stored.first_week;

    for custom in stored.custom_exercises {
        let draft = ExerciseDraft {
            name: custom.name.clone(),
            primary: custom.primary_muscles,
            secondary: custom.secondary_muscles,
            note: custom.note,
        };
        if let Err(err) = tracker.catalog.add_custom(draft) {
            warnings.push(format!("Skipped custom exercise '{}': {err}", custom.name));
        }
    }
    for (name, note) in stored.notes {
        tracker.catalog.set_note(&name, &note);
    }
    tracker.set_recents(stored.recent_exercises);
    tracker.log = TrainingLog::from_weeks(stored.weekly_sets);
    Loaded { tracker, warnings }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Change, ExerciseDraft, Sets};

    fn temp_path(name: &str) -> PathBuf {
        let dir = env::temp_dir().join(format!("puresets-test-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir.join("nested").join("data.json")
    }

    fn sample() -> Tracker {
        let mut tracker = Tracker::new(Catalog::built_in().unwrap());
        let week = Week::new(2026, 44).unwrap();
        tracker.target = Some(Target::new(10).unwrap());
        tracker.first_week = Some(week);
        tracker
            .catalog
            .add_custom(ExerciseDraft {
                name: "Cable Fly".into(),
                primary: vec![Muscle::Chest],
                secondary: vec![Muscle::FrontDelts],
                note: "low pulley".into(),
            })
            .unwrap();
        tracker.catalog.set_note("Squat", "belt");
        tracker.log_sets(week, "Cable Fly", 3).unwrap();
        tracker
            .log
            .adjust(week, Muscle::Abs, Change::Add(Sets::HALF));
        tracker
    }

    #[test]
    fn round_trips_through_disk() {
        let storage = Storage::new(temp_path("round-trip"));
        let tracker = sample();
        storage.save(&tracker).unwrap();
        let loaded = storage.load(Catalog::built_in().unwrap()).unwrap();
        assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
        assert_eq!(loaded.tracker, tracker);
    }

    #[test]
    fn writes_readable_json() {
        let json = serde_json::to_value(snapshot(&sample())).unwrap();
        assert_eq!(json["targetSets"], 10);
        assert_eq!(json["weeklySets"]["2026-W44"]["frontDelts"], 1.5);
        assert_eq!(json["customExercises"][0]["primaryMuscles"][0], "chest");
        assert_eq!(json["notes"]["Squat"], "belt");
    }

    #[test]
    fn missing_file_starts_fresh() {
        let storage = Storage::new(temp_path("missing"));
        let loaded = storage.load(Catalog::built_in().unwrap()).unwrap();
        assert_eq!(loaded.tracker.target, None);
    }

    #[test]
    fn corrupt_file_is_an_error() {
        let path = temp_path("corrupt");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "{ not json").unwrap();
        assert!(
            Storage::new(path)
                .load(Catalog::built_in().unwrap())
                .is_err()
        );
    }

    #[test]
    fn invalid_entries_become_warnings() {
        let stored: StoredData = serde_json::from_str(
            r#"{
                "version": 1,
                "targetSets": 99,
                "customExercises": [{ "name": "Squat", "primaryMuscles": ["quads"] }]
            }"#,
        )
        .unwrap();
        let loaded = restore(stored, Catalog::built_in().unwrap());
        assert_eq!(loaded.tracker.target, None);
        assert_eq!(loaded.warnings.len(), 2, "{:?}", loaded.warnings);
    }
}
