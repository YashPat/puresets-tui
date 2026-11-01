use std::fmt;

use serde::{Deserialize, Serialize};

/// A trackable muscle group.
///
/// Variants are declared in display order (grouped by [`Category`]), so the
/// derived `Ord` sorts muscles the way every screen lists them. The serialized
/// names match the iOS app's `MuscleName` raw values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Muscle {
    Chest,
    Triceps,
    FrontDelts,
    LateralDelts,
    Forearms,
    Biceps,
    Lats,
    UpperBack,
    LowerBack,
    RearDelts,
    Quads,
    Glutes,
    Hamstrings,
    Calves,
    Abs,
}

impl Muscle {
    /// Every muscle, in display order.
    pub const ALL: [Self; 15] = [
        Self::Chest,
        Self::Triceps,
        Self::FrontDelts,
        Self::LateralDelts,
        Self::Forearms,
        Self::Biceps,
        Self::Lats,
        Self::UpperBack,
        Self::LowerBack,
        Self::RearDelts,
        Self::Quads,
        Self::Glutes,
        Self::Hamstrings,
        Self::Calves,
        Self::Abs,
    ];

    /// Human-readable name, as used in `exercises.json`.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Chest => "Chest",
            Self::Triceps => "Triceps",
            Self::FrontDelts => "Front Delts",
            Self::LateralDelts => "Lateral Delts",
            Self::Forearms => "Forearms",
            Self::Biceps => "Biceps",
            Self::Lats => "Lats",
            Self::UpperBack => "Upper Back",
            Self::LowerBack => "Lower Back",
            Self::RearDelts => "Rear Delts",
            Self::Quads => "Quads",
            Self::Glutes => "Glutes",
            Self::Hamstrings => "Hamstrings",
            Self::Calves => "Calves",
            Self::Abs => "Abs",
        }
    }

    pub const fn category(self) -> Category {
        match self {
            Self::Chest | Self::Triceps | Self::FrontDelts | Self::LateralDelts => Category::Push,
            Self::Forearms
            | Self::Biceps
            | Self::Lats
            | Self::UpperBack
            | Self::LowerBack
            | Self::RearDelts => Category::Pull,
            Self::Quads | Self::Glutes | Self::Hamstrings | Self::Calves => Category::Legs,
            Self::Abs => Category::Core,
        }
    }

    /// Looks up a muscle by its human-readable name, ignoring case.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|muscle| muscle.name().eq_ignore_ascii_case(name.trim()))
    }

    /// Position of this muscle in [`Muscle::ALL`].
    pub fn index(self) -> usize {
        Self::ALL
            .iter()
            .position(|&muscle| muscle == self)
            .expect("Muscle::ALL lists every variant")
    }
}

impl fmt::Display for Muscle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// The movement family a muscle belongs to; the home screen groups by it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Category {
    Push,
    Pull,
    Legs,
    Core,
}

impl Category {
    pub const ALL: [Self; 4] = [Self::Push, Self::Pull, Self::Legs, Self::Core];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Push => "Push",
            Self::Pull => "Pull",
            Self::Legs => "Legs",
            Self::Core => "Core",
        }
    }

    /// The muscles in this category, in display order.
    pub fn muscles(self) -> impl Iterator<Item = Muscle> {
        Muscle::ALL
            .into_iter()
            .filter(move |muscle| muscle.category() == self)
    }
}

impl fmt::Display for Category {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_is_sorted_and_grouped_by_category() {
        let mut sorted = Muscle::ALL;
        sorted.sort();
        assert_eq!(sorted, Muscle::ALL);

        let categories: Vec<Category> = Muscle::ALL.iter().map(|m| m.category()).collect();
        let mut grouped = categories;
        grouped.dedup();
        assert_eq!(
            grouped,
            Category::ALL,
            "each category is one contiguous run"
        );
    }

    #[test]
    fn names_round_trip() {
        for muscle in Muscle::ALL {
            assert_eq!(Muscle::from_name(muscle.name()), Some(muscle));
        }
        assert_eq!(Muscle::from_name("front delts"), Some(Muscle::FrontDelts));
        assert_eq!(Muscle::from_name("Shoulders"), None);
    }

    #[test]
    fn index_matches_position() {
        for (i, muscle) in Muscle::ALL.into_iter().enumerate() {
            assert_eq!(muscle.index(), i);
        }
    }

    #[test]
    fn serializes_like_the_ios_app() {
        let json = serde_json::to_string(&Muscle::FrontDelts).unwrap();
        assert_eq!(json, "\"frontDelts\"");
    }
}
