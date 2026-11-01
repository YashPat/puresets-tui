use std::fmt;

use serde::Deserialize;

use super::{Muscle, Sets};

/// The exercise list shipped with the iOS app, embedded at build time.
const BUILT_IN_JSON: &str = include_str!("../../data/exercises.json");

/// Whether an exercise ships with the app or was created by the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    BuiltIn,
    Custom,
}

/// How strongly an exercise works a muscle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// Credited one full set per logged set.
    Primary,
    /// Credited half a set per logged set.
    Secondary,
}

impl Role {
    /// Credit earned per logged set.
    pub const fn credit(self) -> Sets {
        match self {
            Self::Primary => Sets::ONE,
            Self::Secondary => Sets::HALF,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Primary => "Primary",
            Self::Secondary => "Secondary",
        }
    }
}

/// One muscle's share of a logged exercise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Credit {
    pub muscle: Muscle,
    pub role: Role,
    pub sets: Sets,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Exercise {
    name: String,
    primary: Vec<Muscle>,
    secondary: Vec<Muscle>,
    origin: Origin,
    note: String,
}

impl Exercise {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn primary(&self) -> &[Muscle] {
        &self.primary
    }

    pub fn secondary(&self) -> &[Muscle] {
        &self.secondary
    }

    pub const fn origin(&self) -> Origin {
        self.origin
    }

    pub fn note(&self) -> &str {
        &self.note
    }

    pub fn works(&self, muscle: Muscle) -> bool {
        self.role_for(muscle).is_some()
    }

    pub fn role_for(&self, muscle: Muscle) -> Option<Role> {
        if self.primary.contains(&muscle) {
            Some(Role::Primary)
        } else if self.secondary.contains(&muscle) {
            Some(Role::Secondary)
        } else {
            None
        }
    }

    /// What logging `count` sets of this exercise credits to each muscle,
    /// primary muscles first.
    pub fn credits(&self, count: u32) -> Vec<Credit> {
        let primary = self.primary.iter().map(|&m| (m, Role::Primary));
        let secondary = self.secondary.iter().map(|&m| (m, Role::Secondary));
        primary
            .chain(secondary)
            .map(|(muscle, role)| Credit {
                muscle,
                role,
                sets: role.credit().saturating_mul(count),
            })
            .collect()
    }

    fn matches(&self, terms: &[String]) -> bool {
        let name = self.name.to_lowercase();
        terms.iter().all(|term| name.contains(term.as_str()))
    }
}

/// A user-defined exercise before validation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExerciseDraft {
    pub name: String,
    pub primary: Vec<Muscle>,
    pub secondary: Vec<Muscle>,
    pub note: String,
}

/// Why an exercise could not be created or removed.
///
/// Messages mirror the iOS app's `ExerciseDataError`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExerciseError {
    EmptyName,
    DuplicateName(String),
    NoPrimaryMuscles,
    OverlappingMuscles,
    CannotRemoveBuiltIn,
    NotFound(String),
}

impl fmt::Display for ExerciseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyName => f.write_str("Exercise name cannot be empty."),
            Self::DuplicateName(name) => write!(f, "An exercise named '{name}' already exists."),
            Self::NoPrimaryMuscles => f.write_str("At least one primary muscle is required."),
            Self::OverlappingMuscles => {
                f.write_str("A muscle cannot be both primary and secondary.")
            }
            Self::CannotRemoveBuiltIn => f.write_str("Built-in exercises cannot be deleted."),
            Self::NotFound(name) => write!(f, "No exercise named '{name}'."),
        }
    }
}

impl std::error::Error for ExerciseError {}

/// `exercises.json` could not be turned into a catalog.
#[derive(Debug)]
pub enum CatalogError {
    Parse(json5::Error),
    NoPrimaryMuscles { exercise: String },
}

impl fmt::Display for CatalogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(_) => f.write_str("could not parse the exercise list"),
            Self::NoPrimaryMuscles { exercise } => {
                write!(f, "exercise {exercise:?} has no recognised primary muscle")
            }
        }
    }
}

impl std::error::Error for CatalogError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Parse(err) => Some(err),
            Self::NoPrimaryMuscles { .. } => None,
        }
    }
}

/// One record of `exercises.json`, in the iOS app's format.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExerciseRecord {
    name: String,
    primary_muscles: Vec<String>,
    #[serde(default)]
    secondary_muscles: Vec<String>,
}

/// Every exercise the user can log, kept sorted by name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Catalog {
    exercises: Vec<Exercise>,
}

impl Catalog {
    /// Builds the catalog from the bundled exercise list.
    ///
    /// # Errors
    ///
    /// Fails if the embedded JSON is malformed; a test keeps that from shipping.
    pub fn built_in() -> Result<Self, CatalogError> {
        Self::from_json(BUILT_IN_JSON)
    }

    /// Parses an exercise list in the iOS app's format.
    ///
    /// The file is read as JSON5 because the app's copy has trailing commas.
    /// Like the app, muscle names it does not track (such as "Core") are
    /// skipped, and so are repeated exercise names.
    ///
    /// # Errors
    ///
    /// Fails on malformed JSON or an exercise with no recognised primary muscle.
    pub fn from_json(json: &str) -> Result<Self, CatalogError> {
        let records: Vec<ExerciseRecord> = json5::from_str(json).map_err(CatalogError::Parse)?;
        let mut catalog = Self {
            exercises: Vec::with_capacity(records.len()),
        };
        for record in records {
            if catalog.get(&record.name).is_some() {
                continue;
            }
            let primary = known_muscles(&record.primary_muscles, &[]);
            if primary.is_empty() {
                return Err(CatalogError::NoPrimaryMuscles {
                    exercise: record.name,
                });
            }
            let secondary = known_muscles(&record.secondary_muscles, &primary);
            catalog.insert(Exercise {
                name: record.name.trim().to_owned(),
                primary,
                secondary,
                origin: Origin::BuiltIn,
                note: String::new(),
            });
        }
        Ok(catalog)
    }

    pub fn len(&self) -> usize {
        self.exercises.len()
    }

    pub fn is_empty(&self) -> bool {
        self.exercises.is_empty()
    }

    /// All exercises, sorted by name.
    pub fn iter(&self) -> std::slice::Iter<'_, Exercise> {
        self.exercises.iter()
    }

    pub fn customs(&self) -> impl Iterator<Item = &Exercise> {
        self.exercises
            .iter()
            .filter(|exercise| exercise.origin == Origin::Custom)
    }

    /// Finds an exercise by name, ignoring case and surrounding whitespace.
    pub fn get(&self, name: &str) -> Option<&Exercise> {
        self.position(name).map(|index| &self.exercises[index])
    }

    /// Exercises whose name contains every word of `query`, optionally only
    /// those that work `muscle`.
    pub fn search(&self, query: &str, muscle: Option<Muscle>) -> Vec<&Exercise> {
        let terms: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
        self.exercises
            .iter()
            .filter(|exercise| muscle.is_none_or(|m| exercise.works(m)))
            .filter(|exercise| exercise.matches(&terms))
            .collect()
    }

    /// Validates and adds a user-defined exercise, returning its final name.
    ///
    /// # Errors
    ///
    /// Rejects an empty or duplicate name, no primary muscle, or a muscle
    /// listed as both primary and secondary.
    pub fn add_custom(&mut self, draft: ExerciseDraft) -> Result<String, ExerciseError> {
        let name = draft.name.trim();
        if name.is_empty() {
            return Err(ExerciseError::EmptyName);
        }
        if draft.primary.is_empty() {
            return Err(ExerciseError::NoPrimaryMuscles);
        }
        if draft.primary.iter().any(|m| draft.secondary.contains(m)) {
            return Err(ExerciseError::OverlappingMuscles);
        }
        if self.get(name).is_some() {
            return Err(ExerciseError::DuplicateName(name.to_owned()));
        }
        let name = name.to_owned();
        self.insert(Exercise {
            name: name.clone(),
            primary: sorted_unique(draft.primary),
            secondary: sorted_unique(draft.secondary),
            origin: Origin::Custom,
            note: draft.note,
        });
        Ok(name)
    }

    /// # Errors
    ///
    /// Built-in exercises cannot be removed.
    pub fn remove_custom(&mut self, name: &str) -> Result<Exercise, ExerciseError> {
        let index = self
            .position(name)
            .ok_or_else(|| ExerciseError::NotFound(name.to_owned()))?;
        if self.exercises[index].origin == Origin::BuiltIn {
            return Err(ExerciseError::CannotRemoveBuiltIn);
        }
        Ok(self.exercises.remove(index))
    }

    /// Replaces an exercise's note. Returns `false` if there is no such exercise.
    pub fn set_note(&mut self, name: &str, note: &str) -> bool {
        let Some(index) = self.position(name) else {
            return false;
        };
        note.trim().clone_into(&mut self.exercises[index].note);
        true
    }

    fn position(&self, name: &str) -> Option<usize> {
        let name = name.trim();
        self.exercises
            .iter()
            .position(|exercise| exercise.name.eq_ignore_ascii_case(name))
    }

    fn insert(&mut self, exercise: Exercise) {
        let key = exercise.name.to_lowercase();
        let index = self
            .exercises
            .partition_point(|existing| existing.name.to_lowercase() < key);
        self.exercises.insert(index, exercise);
    }
}

impl<'a> IntoIterator for &'a Catalog {
    type Item = &'a Exercise;
    type IntoIter = std::slice::Iter<'a, Exercise>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// Resolves muscle names, dropping unknown ones and any in `exclude`.
fn known_muscles(names: &[String], exclude: &[Muscle]) -> Vec<Muscle> {
    let mut muscles: Vec<Muscle> = names
        .iter()
        .filter_map(|name| Muscle::from_name(name))
        .filter(|muscle| !exclude.contains(muscle))
        .collect();
    // Keep the file's order (it lists the main mover first), minus repeats.
    let mut seen = Vec::with_capacity(muscles.len());
    muscles.retain(|muscle| {
        let first = !seen.contains(muscle);
        seen.push(*muscle);
        first
    });
    muscles
}

fn sorted_unique(mut muscles: Vec<Muscle>) -> Vec<Muscle> {
    muscles.sort_unstable();
    muscles.dedup();
    muscles
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft(name: &str, primary: &[Muscle], secondary: &[Muscle]) -> ExerciseDraft {
        ExerciseDraft {
            name: name.to_owned(),
            primary: primary.to_vec(),
            secondary: secondary.to_vec(),
            note: String::new(),
        }
    }

    #[test]
    fn bundled_list_loads() {
        let catalog = Catalog::built_in().expect("bundled exercises.json parses");
        assert!(catalog.len() > 40, "got {} exercises", catalog.len());

        let bench = catalog.get("bench press").unwrap();
        assert_eq!(bench.primary(), [Muscle::Chest, Muscle::FrontDelts]);
        assert_eq!(bench.secondary(), [Muscle::Triceps]);
        assert_eq!(bench.origin(), Origin::BuiltIn);
    }

    #[test]
    fn bundled_list_is_sorted_and_unique() {
        let catalog = Catalog::built_in().unwrap();
        let names: Vec<String> = catalog.iter().map(|e| e.name().to_lowercase()).collect();
        let mut sorted = names.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(
            names, sorted,
            "the file repeats \"Chest Dips\"; it is kept once"
        );
    }

    #[test]
    fn unknown_muscles_are_skipped_like_the_ios_app() {
        let catalog = Catalog::built_in().unwrap();
        // "Overhead Press" lists "Shoulders" and "Core", which are not tracked.
        let press = catalog.get("Overhead Press").unwrap();
        assert_eq!(press.primary(), [Muscle::Triceps]);
        assert_eq!(press.secondary(), [Muscle::Chest]);
    }

    #[test]
    fn rejects_exercise_without_primary_muscle() {
        let json = r#"[{ "name": "Mystery", "primaryMuscles": ["Core"] }]"#;
        assert!(matches!(
            Catalog::from_json(json),
            Err(CatalogError::NoPrimaryMuscles { .. })
        ));
    }

    #[test]
    fn credits_half_sets_to_secondary_muscles() {
        let catalog = Catalog::built_in().unwrap();
        let credits = catalog.get("Bench Press").unwrap().credits(3);
        let summary: Vec<(Muscle, String)> = credits
            .iter()
            .map(|c| (c.muscle, c.sets.to_string()))
            .collect();
        assert_eq!(
            summary,
            [
                (Muscle::Chest, "3".to_owned()),
                (Muscle::FrontDelts, "3".to_owned()),
                (Muscle::Triceps, "1.5".to_owned()),
            ]
        );
    }

    #[test]
    fn search_matches_every_word_and_filters_by_muscle() {
        let catalog = Catalog::built_in().unwrap();
        let names = |query, muscle| -> Vec<String> {
            catalog
                .search(query, muscle)
                .iter()
                .map(|e| e.name().to_owned())
                .collect()
        };
        assert_eq!(names("BENCH inc", None), ["Incline Bench Press".to_owned()]);
        assert!(names("curl", Some(Muscle::Biceps)).len() >= 4);
        assert!(names("curl", Some(Muscle::Quads)).is_empty());
        assert_eq!(names("", None).len(), catalog.len());
    }

    #[test]
    fn custom_exercise_validation() {
        let mut catalog = Catalog::built_in().unwrap();
        let cases = [
            (draft("  ", &[Muscle::Chest], &[]), ExerciseError::EmptyName),
            (
                draft("Cable Fly", &[], &[]),
                ExerciseError::NoPrimaryMuscles,
            ),
            (
                draft("Cable Fly", &[Muscle::Chest], &[Muscle::Chest]),
                ExerciseError::OverlappingMuscles,
            ),
            (
                draft("bench press", &[Muscle::Chest], &[]),
                ExerciseError::DuplicateName("bench press".to_owned()),
            ),
        ];
        for (input, want) in cases {
            assert_eq!(catalog.add_custom(input.clone()), Err(want), "{input:?}");
        }
    }

    #[test]
    fn custom_exercises_can_be_added_and_removed() {
        let mut catalog = Catalog::built_in().unwrap();
        let before = catalog.len();
        let name = catalog
            .add_custom(draft(
                " Cable Fly ",
                &[Muscle::Chest],
                &[Muscle::FrontDelts],
            ))
            .unwrap();
        assert_eq!(name, "Cable Fly");
        assert_eq!(catalog.len(), before + 1);
        assert_eq!(catalog.customs().count(), 1);

        assert_eq!(
            catalog.remove_custom("Bench Press"),
            Err(ExerciseError::CannotRemoveBuiltIn)
        );
        assert_eq!(
            catalog.remove_custom("cable fly").unwrap().name(),
            "Cable Fly"
        );
        assert_eq!(catalog.len(), before);
    }

    #[test]
    fn notes_are_trimmed() {
        let mut catalog = Catalog::built_in().unwrap();
        assert!(catalog.set_note("squat", "  brace core "));
        assert_eq!(catalog.get("Squat").unwrap().note(), "brace core");
        assert!(!catalog.set_note("nope", "x"));
    }
}
