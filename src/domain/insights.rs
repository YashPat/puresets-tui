use std::fmt;

use super::{Category, Muscle, Sets, TrainingLog, Week};

/// Weekly hard-set target per muscle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Target(u8);

impl Target {
    pub const MIN: u8 = 4;
    pub const MAX: u8 = 20;

    /// # Errors
    ///
    /// Returns the rejected value if it is outside `MIN..=MAX`.
    pub const fn new(sets: u8) -> Result<Self, u8> {
        if sets >= Self::MIN && sets <= Self::MAX {
            Ok(Self(sets))
        } else {
            Err(sets)
        }
    }

    pub const fn get(self) -> u8 {
        self.0
    }

    pub fn sets(self) -> Sets {
        Sets::whole(u32::from(self.0))
    }

    /// One step up or down, clamped to the allowed range.
    #[must_use]
    pub const fn step(self, direction: Direction) -> Self {
        match direction {
            Direction::Up if self.0 < Self::MAX => Self(self.0 + 1),
            Direction::Down if self.0 > Self::MIN => Self(self.0 - 1),
            Direction::Up | Direction::Down => self,
        }
    }

    /// The preset this target matches, if any.
    pub fn goal(self) -> Option<Goal> {
        Goal::ALL.into_iter().find(|goal| goal.target() == self)
    }
}

impl Default for Target {
    fn default() -> Self {
        Goal::Moderate.target()
    }
}

impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
}

/// The two presets offered when the app is first opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Goal {
    Moderate,
    Optimal,
}

impl Goal {
    pub const ALL: [Self; 2] = [Self::Moderate, Self::Optimal];

    pub const fn target(self) -> Target {
        match self {
            Self::Moderate => Target(8),
            Self::Optimal => Target(12),
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Moderate => "Moderate",
            Self::Optimal => "Optimal",
        }
    }

    pub const fn blurb(self) -> &'static str {
        match self {
            Self::Moderate => "Solid growth on a busy schedule. Most of the benefit, less time.",
            Self::Optimal => "More volume for more growth, if you can recover from it.",
        }
    }
}

/// How a muscle's sets compare with the target. Thresholds match the iOS app.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProgressLevel {
    /// Under half the target.
    Low,
    /// 50% to 75% of the target.
    Medium,
    /// 75% up to and including the target.
    Good,
    /// Up to 25% over the target.
    Excellent,
    /// More than 25% over the target.
    Overkill,
}

impl ProgressLevel {
    pub fn of(sets: Sets, target: Target) -> Self {
        // In half-sets the target is `2t`, so 50% is `t`, 75% is `1.5t` and
        // 125% is `2.5t`. Doubling both sides keeps the comparison exact.
        let doubled = u64::from(sets.halves()) * 2;
        let t = u64::from(target.get());
        if doubled < 2 * t {
            Self::Low
        } else if doubled < 3 * t {
            Self::Medium
        } else if doubled <= 4 * t {
            Self::Good
        } else if doubled <= 5 * t {
            Self::Excellent
        } else {
            Self::Overkill
        }
    }
}

/// The time span the insights screen summarises, ending with the current week.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Period {
    #[default]
    ThisWeek,
    FourWeeks,
    Year,
}

impl Period {
    pub const ALL: [Self; 3] = [Self::ThisWeek, Self::FourWeeks, Self::Year];

    pub const fn weeks(self) -> usize {
        match self {
            Self::ThisWeek => 1,
            Self::FourWeeks => 4,
            Self::Year => 52,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::ThisWeek => "This week",
            Self::FourWeeks => "4 weeks",
            Self::Year => "52 weeks",
        }
    }

    #[must_use]
    pub fn next(self) -> Self {
        let index = Self::ALL.iter().position(|&p| p == self).unwrap_or(0);
        Self::ALL[(index + 1) % Self::ALL.len()]
    }

    #[must_use]
    pub fn previous(self) -> Self {
        let index = Self::ALL.iter().position(|&p| p == self).unwrap_or(0);
        Self::ALL[(index + Self::ALL.len() - 1) % Self::ALL.len()]
    }
}

/// Totals for one category over a span of weeks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CategorySummary {
    pub category: Category,
    pub total: Sets,
    /// Average weekly sets per muscle.
    pub average: Sets,
    /// Share of the combined target met, counting each muscle up to its target.
    pub completion: u8,
    pub below_target: usize,
}

/// Per-muscle and per-category figures for a span of weeks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Insights {
    pub weeks: Vec<Week>,
    /// Average weekly sets per muscle, in display order.
    pub muscles: Vec<(Muscle, Sets)>,
    pub categories: Vec<CategorySummary>,
    pub total: Sets,
}

impl Insights {
    /// Summarises the `period` ending with `current`.
    ///
    /// Averages are per week, rounded down to the nearest half set, so a
    /// four-week view compares directly with the weekly target.
    pub fn compute(log: &TrainingLog, current: Week, period: Period, target: Target) -> Self {
        let weeks: Vec<Week> = current.trailing(period.weeks()).collect();
        let week_count = u32::try_from(weeks.len()).unwrap_or(u32::MAX).max(1);
        let average = |total: Sets| Sets::from_halves(total.halves() / week_count);

        let muscles: Vec<(Muscle, Sets)> = Muscle::ALL
            .into_iter()
            .map(|muscle| (muscle, average(log.total_for(muscle, &weeks))))
            .collect();

        let categories = Category::ALL
            .into_iter()
            .map(|category| {
                let members: Vec<(Muscle, Sets)> = muscles
                    .iter()
                    .copied()
                    .filter(|(muscle, _)| muscle.category() == category)
                    .collect();
                summarise(category, &members, &weeks, log, target)
            })
            .collect();

        let total = muscles
            .iter()
            .map(|&(muscle, _)| log.total_for(muscle, &weeks))
            .sum();
        Self {
            weeks,
            muscles,
            categories,
            total,
        }
    }

    /// Muscles sorted by average sets, most trained first.
    pub fn ranked(&self) -> Vec<(Muscle, Sets)> {
        let mut ranked = self.muscles.clone();
        ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        ranked
    }
}

fn summarise(
    category: Category,
    members: &[(Muscle, Sets)],
    weeks: &[Week],
    log: &TrainingLog,
    target: Target,
) -> CategorySummary {
    let total: Sets = members
        .iter()
        .map(|&(muscle, _)| log.total_for(muscle, weeks))
        .sum();
    let member_count = u32::try_from(members.len()).unwrap_or(u32::MAX).max(1);
    let average_sum: Sets = members.iter().map(|&(_, sets)| sets).sum();
    let capped: u64 = members
        .iter()
        .map(|&(_, sets)| u64::from(sets.min(target.sets()).halves()))
        .sum();
    let goal = u64::from(target.sets().halves()) * u64::from(member_count);
    let completion = u8::try_from(capped * 100 / goal.max(1)).unwrap_or(100);
    CategorySummary {
        category,
        total,
        average: Sets::from_halves(average_sum.halves() / member_count),
        completion,
        below_target: members
            .iter()
            .filter(|&&(_, sets)| sets < target.sets())
            .count(),
    }
}

/// One point of the dose-response curve.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DosePoint {
    pub sets: f64,
    pub gain: f64,
    pub lower: f64,
    pub upper: f64,
}

const DOSE_RESPONSE_CSV: &str = include_str!("../../data/hypertrophy-dose-response.csv");

/// The modelled hypertrophy curve bundled with the iOS app (weekly sets
/// against expected percent muscle growth, with a credible interval).
///
/// Rows that do not parse are skipped; a test checks the bundled file.
pub fn dose_response() -> Vec<DosePoint> {
    DOSE_RESPONSE_CSV
        .lines()
        .skip(1)
        .filter_map(|line| {
            let mut fields = line.split(',').map(|field| field.trim().parse::<f64>());
            let mut next = || fields.next()?.ok();
            Some(DosePoint {
                sets: next()?,
                gain: next()?,
                lower: next()?,
                upper: next()?,
            })
        })
        .collect()
}

/// Consecutive weeks under half the target before a muscle is flagged.
pub const NEGLECTED_MIN_WEEKS: usize = 3;

/// A muscle that has stayed under half its weekly target for several weeks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NeglectedMuscle {
    pub muscle: Muscle,
    /// Consecutive weeks at [`ProgressLevel::Low`], ending with the current week.
    pub weeks: usize,
}

/// Muscles at [`ProgressLevel::Low`] for at least [`NEGLECTED_MIN_WEEKS`]
/// consecutive weeks ending with `current`, longest streak first (ties in
/// muscle order).
///
/// Weeks before the log's first week never count, so a new user sees nothing.
pub fn neglected_muscles(log: &TrainingLog, current: Week, target: Target) -> Vec<NeglectedMuscle> {
    let Some(first) = log.earliest_week() else {
        return Vec::new();
    };
    let mut flagged: Vec<NeglectedMuscle> = Muscle::ALL
        .into_iter()
        .map(|muscle| NeglectedMuscle {
            muscle,
            weeks: std::iter::successors(Some(current), |week| Some(week.previous()))
                .take_while(|&week| week >= first)
                .take_while(|&week| {
                    ProgressLevel::of(log.sets(week, muscle), target) == ProgressLevel::Low
                })
                .count(),
        })
        .filter(|neglected| neglected.weeks >= NEGLECTED_MIN_WEEKS)
        .collect();
    // Stable sort keeps muscle order among equal streaks.
    flagged.sort_by_key(|n| std::cmp::Reverse(n.weeks));
    flagged
}

#[cfg(test)]
mod neglected_tests {
    use super::*;
    use crate::domain::Change;

    fn target() -> Target {
        Target::new(8).unwrap()
    }

    fn week(n: i8) -> Week {
        Week::new(2026, n).unwrap()
    }

    fn add(log: &mut TrainingLog, week: Week, muscle: Muscle, sets: u32) {
        log.adjust(week, muscle, Change::Add(Sets::whole(sets)));
    }

    /// Logs `sets` of Chest in every week from `from` through `to`.
    fn chest_weeks(log: &mut TrainingLog, from: i8, to: i8, sets: u32) {
        for n in from..=to {
            add(log, week(n), Muscle::Chest, sets);
        }
    }

    fn streak(flagged: &[NeglectedMuscle], muscle: Muscle) -> Option<usize> {
        flagged.iter().find(|n| n.muscle == muscle).map(|n| n.weeks)
    }

    #[test]
    fn empty_log_flags_nothing() {
        assert!(neglected_muscles(&TrainingLog::new(), week(44), target()).is_empty());
    }

    #[test]
    fn new_user_is_not_flagged() {
        let mut log = TrainingLog::new();
        add(&mut log, week(44), Muscle::Chest, 8);
        assert!(neglected_muscles(&log, week(44), target()).is_empty());
    }

    #[test]
    fn streak_is_capped_by_the_first_logged_week() {
        let mut log = TrainingLog::new();
        chest_weeks(&mut log, 40, 44, 8);
        let flagged = neglected_muscles(&log, week(44), target());
        // Weeks 40 to 44 count; the weeks before week 40 do not.
        assert_eq!(streak(&flagged, Muscle::Biceps), Some(5));
        assert_eq!(streak(&flagged, Muscle::Chest), None);
    }

    #[test]
    fn two_weeks_is_not_flagged() {
        let mut log = TrainingLog::new();
        chest_weeks(&mut log, 43, 44, 8);
        assert!(neglected_muscles(&log, week(44), target()).is_empty());
    }

    #[test]
    fn exactly_three_weeks_is_flagged() {
        let mut log = TrainingLog::new();
        chest_weeks(&mut log, 42, 44, 8);
        let flagged = neglected_muscles(&log, week(44), target());
        assert_eq!(streak(&flagged, Muscle::Biceps), Some(NEGLECTED_MIN_WEEKS));
    }

    #[test]
    fn a_good_week_breaks_the_streak() {
        let mut log = TrainingLog::new();
        chest_weeks(&mut log, 38, 44, 8);
        // Biceps: low in 38-40, fine in 41, low again in 42-44.
        add(&mut log, week(41), Muscle::Biceps, 8);
        let flagged = neglected_muscles(&log, week(44), target());
        assert_eq!(streak(&flagged, Muscle::Biceps), Some(3));
        // A fine current week ends the streak entirely.
        add(&mut log, week(44), Muscle::Biceps, 4);
        let flagged = neglected_muscles(&log, week(44), target());
        assert_eq!(streak(&flagged, Muscle::Biceps), None);
    }

    #[test]
    fn half_the_target_is_not_low() {
        let mut log = TrainingLog::new();
        chest_weeks(&mut log, 40, 44, 8);
        for n in 40..=44 {
            add(&mut log, week(n), Muscle::Calves, 4);
        }
        let flagged = neglected_muscles(&log, week(44), target());
        assert_eq!(streak(&flagged, Muscle::Calves), None);
    }

    #[test]
    fn longest_streak_first_then_muscle_order() {
        let mut log = TrainingLog::new();
        chest_weeks(&mut log, 40, 44, 8);
        // Biceps and Triceps are fine in weeks 40-41, leaving streaks of 3;
        // every other muscle except Chest has the full 5.
        for n in 40..=41 {
            add(&mut log, week(n), Muscle::Triceps, 8);
            add(&mut log, week(n), Muscle::Biceps, 8);
        }
        let flagged = neglected_muscles(&log, week(44), target());
        let expected: Vec<(Muscle, usize)> = Muscle::ALL
            .into_iter()
            .filter(|&m| m != Muscle::Chest && !matches!(m, Muscle::Biceps | Muscle::Triceps))
            .map(|m| (m, 5))
            .chain([(Muscle::Triceps, 3), (Muscle::Biceps, 3)])
            .collect();
        let got: Vec<(Muscle, usize)> = flagged.iter().map(|n| (n.muscle, n.weeks)).collect();
        assert_eq!(got, expected);
    }
}

/// How each muscle fared in each of the last `weeks` weeks, for a heatmap.
///
/// Muscles come in [`Muscle::ALL`] order; each row runs oldest to newest and
/// ends with `current`. Weeks before `first_week` are `None` ("no data"), not
/// [`ProgressLevel::Low`], because nothing was being tracked yet. With no
/// `first_week` every cell is `None`.
pub fn muscle_week_levels(
    log: &TrainingLog,
    first_week: Option<Week>,
    current: Week,
    weeks: usize,
    target: Target,
) -> Vec<(Muscle, Vec<Option<ProgressLevel>>)> {
    let span: Vec<Week> = current.trailing(weeks).collect();
    Muscle::ALL
        .into_iter()
        .map(|muscle| {
            let levels = span
                .iter()
                .rev()
                .map(|&week| {
                    first_week
                        .filter(|&first| week >= first)
                        .map(|_| ProgressLevel::of(log.sets(week, muscle), target))
                })
                .collect();
            (muscle, levels)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Change;

    fn target(n: u8) -> Target {
        Target::new(n).unwrap()
    }

    #[test]
    fn target_is_bounded() {
        assert!(Target::new(3).is_err());
        assert!(Target::new(21).is_err());
        assert_eq!(target(20).step(Direction::Up), target(20));
        assert_eq!(target(4).step(Direction::Down), target(4));
        assert_eq!(target(8).step(Direction::Up), target(9));
        assert_eq!(target(12).goal(), Some(Goal::Optimal));
        assert_eq!(target(10).goal(), None);
    }

    #[test]
    fn progress_levels_match_the_ios_thresholds() {
        let level = |halves| ProgressLevel::of(Sets::from_halves(halves), target(8));
        assert_eq!(level(0), ProgressLevel::Low);
        assert_eq!(level(7), ProgressLevel::Low); // 3.5 of 8
        assert_eq!(level(8), ProgressLevel::Medium); // 4 = 50%
        assert_eq!(level(12), ProgressLevel::Good); // 6 = 75%
        assert_eq!(level(16), ProgressLevel::Good); // 8 = 100%
        assert_eq!(level(20), ProgressLevel::Excellent); // 10 = 125%
        assert_eq!(level(21), ProgressLevel::Overkill);
    }

    #[test]
    fn period_cycles() {
        assert_eq!(Period::Year.next(), Period::ThisWeek);
        assert_eq!(Period::ThisWeek.previous(), Period::Year);
    }

    #[test]
    fn category_summary() {
        let week = Week::new(2026, 44).unwrap();
        let mut log = TrainingLog::new();
        log.adjust(week, Muscle::Chest, Change::Add(Sets::whole(12)));
        log.adjust(week, Muscle::Triceps, Change::Add(Sets::whole(4)));

        let insights = Insights::compute(&log, week, Period::ThisWeek, target(8));
        let push = insights.categories[0];
        assert_eq!(push.category, Category::Push);
        assert_eq!(push.total, Sets::whole(16));
        assert_eq!(push.average, Sets::whole(4));
        // Chest counts 8 (capped) + triceps 4 = 12 of 32.
        assert_eq!(push.completion, 37);
        assert_eq!(push.below_target, 3);
        assert_eq!(insights.ranked()[0], (Muscle::Chest, Sets::whole(12)));
        assert_eq!(insights.total, Sets::whole(16));
    }

    #[test]
    fn multi_week_averages_are_per_week() {
        let week = Week::new(2026, 44).unwrap();
        let mut log = TrainingLog::new();
        log.adjust(week, Muscle::Quads, Change::Add(Sets::whole(10)));
        log.adjust(week.previous(), Muscle::Quads, Change::Add(Sets::whole(6)));
        let insights = Insights::compute(&log, week, Period::FourWeeks, target(8));
        let quads = insights.muscles[Muscle::Quads.index()];
        assert_eq!(quads, (Muscle::Quads, Sets::whole(4)));
    }

    #[test]
    fn bundled_dose_response_parses() {
        let curve = dose_response();
        assert_eq!(curve.len(), 46);
        assert!(
            curve
                .windows(2)
                .all(|w| w[0].sets < w[1].sets && w[0].gain <= w[1].gain)
        );
        assert!(curve.iter().all(|p| p.lower <= p.gain && p.gain <= p.upper));
    }
}

#[cfg(test)]
mod heatmap_tests {
    use super::*;
    use crate::domain::Change;

    fn target() -> Target {
        Target::new(8).unwrap()
    }

    fn week() -> Week {
        Week::new(2026, 44).unwrap()
    }

    fn row(
        levels: &[(Muscle, Vec<Option<ProgressLevel>>)],
        muscle: Muscle,
    ) -> &[Option<ProgressLevel>] {
        &levels.iter().find(|(m, _)| *m == muscle).unwrap().1
    }

    #[test]
    fn an_untracked_log_has_no_data_anywhere() {
        let levels = muscle_week_levels(&TrainingLog::new(), None, week(), 12, target());
        assert_eq!(levels.len(), Muscle::ALL.len());
        assert!(levels.iter().all(|(_, row)| row.len() == 12));
        assert!(levels.iter().flat_map(|(_, row)| row).all(Option::is_none));
    }

    #[test]
    fn muscles_follow_the_display_order() {
        let levels = muscle_week_levels(&TrainingLog::new(), None, week(), 3, target());
        let muscles: Vec<Muscle> = levels.iter().map(|(muscle, _)| *muscle).collect();
        assert_eq!(muscles, Muscle::ALL);
    }

    #[test]
    fn weeks_before_the_first_are_no_data_and_idle_weeks_after_are_low() {
        let first = week().previous().previous();
        let levels = muscle_week_levels(&TrainingLog::new(), Some(first), week(), 5, target());
        let low = Some(ProgressLevel::Low);
        assert_eq!(
            row(&levels, Muscle::Chest),
            [None, None, low, low, low],
            "two untracked weeks, then three tracked weeks with nothing logged"
        );
    }

    #[test]
    fn each_tier_shows_and_the_row_runs_oldest_to_newest() {
        let mut log = TrainingLog::new();
        let mut oldest = week();
        for _ in 0..4 {
            oldest = oldest.previous();
        }
        // Target 8: 3 low, 4 medium, 6 good, 10 excellent, 11 overkill.
        let sets = [3, 4, 6, 10, 11];
        let mut at = oldest;
        for count in sets {
            log.adjust(at, Muscle::Chest, Change::Add(Sets::whole(count)));
            at = at.next();
        }
        let levels = muscle_week_levels(&log, log.earliest_week(), week(), 5, target());
        assert_eq!(
            row(&levels, Muscle::Chest),
            [
                Some(ProgressLevel::Low),
                Some(ProgressLevel::Medium),
                Some(ProgressLevel::Good),
                Some(ProgressLevel::Excellent),
                Some(ProgressLevel::Overkill),
            ]
        );
        assert_eq!(row(&levels, Muscle::Lats), [Some(ProgressLevel::Low); 5]);
    }
}
