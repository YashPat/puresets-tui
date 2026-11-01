//! The tracker's rules, independent of the terminal: muscles, exercises,
//! weekly set totals and the summaries built from them.

mod catalog;
mod insights;
mod log;
mod muscle;
mod sets;
mod tracker;
mod week;

pub use catalog::{Catalog, Credit, Exercise, ExerciseDraft, Origin, Role};
pub use insights::{
    CategorySummary, Direction, DosePoint, Goal, Insights, Period, ProgressLevel, Target,
    dose_response,
};
pub use log::{Change, TrainingLog, WeekSets};
pub use muscle::{Category, Muscle};
pub use sets::Sets;
pub use tracker::Tracker;
pub use week::Week;
