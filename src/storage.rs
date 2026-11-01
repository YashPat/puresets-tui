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
    env,
    ffi::OsString,
    fmt, fs, io,
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

/// The most `<name>.bak` files kept before an unreadable data file is left
/// in place with an error instead.
const MAX_BACKUPS: u32 = 99;

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

/// Where the data file lives, and what it held when last read or written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Storage {
    path: PathBuf,
    /// The file's contents as last loaded or saved; `None` if it did not exist.
    on_disk: Option<String>,
}

/// A tracker read from disk, plus anything that had to be skipped.
#[derive(Debug)]
pub struct Loaded {
    pub tracker: Tracker,
    pub warnings: Vec<String>,
    /// Where an unparseable data file was moved before starting fresh.
    pub backup: Option<PathBuf>,
}

/// What [`Storage::load`] does with a data file that is not valid JSON.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unparseable {
    /// Report an error and leave the file alone.
    Fail,
    /// Rename it to `<name>.bak` and start with empty data.
    BackUp,
}

/// Why [`Storage::save`] did not write the data file.
#[derive(Debug)]
pub enum SaveError {
    /// Something else changed or removed the file since it was loaded or
    /// last saved, so writing would lose those changes.
    ChangedOnDisk(PathBuf),
    Io(color_eyre::Report),
}

impl fmt::Display for SaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ChangedOnDisk(path) => write!(
                f,
                "{} changed outside PureSets, so it was not overwritten",
                path.display()
            ),
            Self::Io(err) => write!(f, "{err:#}"),
        }
    }
}

impl std::error::Error for SaveError {}

impl Storage {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            on_disk: None,
        }
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
    /// Fails if the file exists but cannot be read, was written by a newer
    /// version, or is not valid JSON and `unparseable` is
    /// [`Unparseable::Fail`]. The caller should stop rather than overwrite it.
    pub fn load(&mut self, catalog: Catalog, unparseable: Unparseable) -> Result<Loaded> {
        let fresh = |catalog, backup| Loaded {
            tracker: Tracker::new(catalog),
            warnings: Vec::new(),
            backup,
        };
        let Some(json) = self.read()? else {
            self.on_disk = None;
            return Ok(fresh(catalog, None));
        };
        let stored: StoredData = match serde_json::from_str(&json) {
            Ok(stored) => stored,
            Err(err) => match unparseable {
                Unparseable::Fail => {
                    return Err(err).wrap_err_with(|| format!("parsing {}", self.path.display()));
                }
                Unparseable::BackUp => {
                    let backup = self.back_up()?;
                    self.on_disk = None;
                    return Ok(fresh(catalog, Some(backup)));
                }
            },
        };
        if stored.version > FORMAT_VERSION {
            bail!(
                "{} was written by a newer PureSets (format {}); this build reads format {FORMAT_VERSION}",
                self.path.display(),
                stored.version
            );
        }
        self.on_disk = Some(json);
        Ok(restore(stored, catalog))
    }

    /// Writes the data file atomically: a temporary file, then a rename.
    ///
    /// # Errors
    ///
    /// Returns [`SaveError::ChangedOnDisk`], without writing, if the file no
    /// longer holds what was last loaded or saved. Otherwise fails if the
    /// directory cannot be created or the file cannot be written.
    pub fn save(&mut self, tracker: &Tracker) -> Result<(), SaveError> {
        if self.read().map_err(SaveError::Io)? != self.on_disk {
            return Err(SaveError::ChangedOnDisk(self.path.clone()));
        }
        let mut json = serde_json::to_string_pretty(&snapshot(tracker))
            .map_err(|err| SaveError::Io(err.into()))?;
        json.push('\n');
        self.write(&json).map_err(SaveError::Io)?;
        self.on_disk = Some(json);
        Ok(())
    }

    /// The file's contents, or `None` if it does not exist.
    fn read(&self) -> Result<Option<String>> {
        match fs::read_to_string(&self.path) {
            Ok(json) => Ok(Some(json)),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(err).wrap_err_with(|| format!("reading {}", self.path.display())),
        }
    }

    fn write(&self, json: &str) -> Result<()> {
        if let Some(dir) = self.path.parent().filter(|dir| !dir.as_os_str().is_empty()) {
            fs::create_dir_all(dir).wrap_err_with(|| format!("creating {}", dir.display()))?;
        }
        let temp = self.path.with_extension("json.tmp");
        fs::write(&temp, json).wrap_err_with(|| format!("writing {}", temp.display()))?;
        fs::rename(&temp, &self.path).wrap_err_with(|| format!("replacing {}", self.path.display()))
    }

    /// Renames the data file to `<name>.bak`, or `<name>.bak.2` and so on if
    /// earlier backups exist, and returns the new path.
    fn back_up(&self) -> Result<PathBuf> {
        let mut name = self
            .path
            .file_name()
            .map_or_else(OsString::new, OsString::from);
        name.push(".bak");
        let first = self.path.with_file_name(&name);
        let backup = std::iter::once(first)
            .chain((2..=MAX_BACKUPS).map(|n| {
                let mut numbered = name.clone();
                numbered.push(format!(".{n}"));
                self.path.with_file_name(numbered)
            }))
            .find(|candidate| !candidate.exists())
            .ok_or_else(|| {
                eyre!(
                    "{} is unreadable and {MAX_BACKUPS} backups of it already exist",
                    self.path.display()
                )
            })?;
        fs::rename(&self.path, &backup).wrap_err_with(|| {
            format!(
                "moving unreadable {} to {}",
                self.path.display(),
                backup.display()
            )
        })?;
        Ok(backup)
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
    Loaded {
        tracker,
        warnings,
        backup: None,
    }
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

    fn load(storage: &mut Storage, unparseable: Unparseable) -> Result<Loaded> {
        storage.load(Catalog::built_in().unwrap(), unparseable)
    }

    #[test]
    fn round_trips_through_disk() {
        let mut storage = Storage::new(temp_path("round-trip"));
        let tracker = sample();
        storage.save(&tracker).unwrap();
        let loaded = load(&mut storage, Unparseable::Fail).unwrap();
        assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
        assert_eq!(loaded.tracker, tracker);
    }

    #[test]
    fn saves_repeatedly_while_nothing_else_writes() {
        let mut storage = Storage::new(temp_path("repeat"));
        load(&mut storage, Unparseable::Fail).unwrap();
        let mut tracker = sample();
        storage.save(&tracker).unwrap();
        tracker.catalog.set_note("Squat", "no belt");
        storage.save(&tracker).unwrap();
        let loaded = load(&mut storage, Unparseable::Fail).unwrap();
        assert_eq!(loaded.tracker, tracker);
    }

    #[test]
    fn does_not_overwrite_a_file_changed_on_disk() {
        let path = temp_path("changed");
        let mut storage = Storage::new(path.clone());
        storage.save(&sample()).unwrap();
        load(&mut storage, Unparseable::Fail).unwrap();
        let original = fs::read_to_string(&path).unwrap();

        let edited = r#"{ "version": 1, "targetSets": 12 }"#;
        fs::write(&path, edited).unwrap();
        let result = storage.save(&sample());
        assert!(
            matches!(result, Err(SaveError::ChangedOnDisk(_))),
            "{result:?}"
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), edited);

        // Putting the file back as it was lets saving resume.
        fs::write(&path, original).unwrap();
        storage.save(&sample()).unwrap();

        fs::remove_file(&path).unwrap();
        assert!(matches!(
            storage.save(&sample()),
            Err(SaveError::ChangedOnDisk(_))
        ));
        assert!(!path.exists(), "a deleted file is not recreated");
    }

    #[test]
    fn does_not_overwrite_a_file_created_after_loading() {
        let path = temp_path("created");
        let mut storage = Storage::new(path.clone());
        load(&mut storage, Unparseable::Fail).unwrap();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "{}").unwrap();
        assert!(matches!(
            storage.save(&sample()),
            Err(SaveError::ChangedOnDisk(_))
        ));
        assert_eq!(fs::read_to_string(&path).unwrap(), "{}");
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
        let mut storage = Storage::new(temp_path("missing"));
        let loaded = load(&mut storage, Unparseable::Fail).unwrap();
        assert_eq!(loaded.tracker.target, None);
        assert_eq!(loaded.backup, None);
    }

    #[test]
    fn corrupt_file_is_an_error_when_asked_to_fail() {
        let path = temp_path("corrupt");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "{ not json").unwrap();
        assert!(load(&mut Storage::new(path.clone()), Unparseable::Fail).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "{ not json");
    }

    #[test]
    fn corrupt_file_is_backed_up_and_replaced() {
        let path = temp_path("backup");
        let dir = path.parent().unwrap();
        fs::create_dir_all(dir).unwrap();
        let mut storage = Storage::new(path.clone());

        for (attempt, backup_name) in [(1, "data.json.bak"), (2, "data.json.bak.2")] {
            let junk = format!("{{ not json {attempt}");
            fs::write(&path, &junk).unwrap();
            let loaded = load(&mut storage, Unparseable::BackUp).unwrap();
            let backup = dir.join(backup_name);
            assert_eq!(loaded.backup.as_deref(), Some(backup.as_path()));
            assert_eq!(fs::read_to_string(&backup).unwrap(), junk, "backup kept");
            assert!(!path.exists(), "corrupt file moved away");
            assert_eq!(loaded.tracker.target, None);
        }

        storage.save(&sample()).unwrap();
        assert!(path.exists(), "the fresh data saves in its place");
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
