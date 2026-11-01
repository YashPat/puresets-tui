//! Short transitions. Each is a start instant and a length, and drawing asks
//! how far along it is at the app's clock, so the frame rate never changes
//! what a frame shows and tests can pin the time.

use std::time::{Duration, Instant};

use crate::domain::Sets;

/// Whether transitions play. With motion off every change shows at once.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Motion {
    #[default]
    On,
    Off,
}

/// Progress through a transition is measured in thousandths.
const WHOLE: u32 = 1000;

/// One transition's timing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tween {
    start: Instant,
    duration: Duration,
}

impl Tween {
    pub const fn new(start: Instant, duration: Duration) -> Self {
        Self { start, duration }
    }

    pub fn is_running(self, now: Instant) -> bool {
        now < self.start + self.duration
    }

    /// Linear progress in thousandths, 0 before the start and 1000 after the end.
    pub fn linear(self, now: Instant) -> u32 {
        let elapsed = now.saturating_duration_since(self.start).as_millis();
        let total = self.duration.as_millis().max(1);
        u32::try_from(elapsed.min(total) * u128::from(WHOLE) / total).unwrap_or(WHOLE)
    }

    /// Eased progress in thousandths: quick to start, gentle to stop (cubic ease-out).
    pub fn eased(self, now: Instant) -> u32 {
        let rest = u64::from(WHOLE - self.linear(now));
        let rest_cubed = rest * rest * rest / u64::from(WHOLE * WHOLE);
        WHOLE - u32::try_from(rest_cubed).unwrap_or(WHOLE)
    }
}

/// How much of a set bar is drawn, in sixteenths of a set: fine enough for
/// eighth-block glyphs whether the bar uses one or two cells per set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Fill(u32);

impl Fill {
    pub const PER_SET: u32 = 16;

    pub const fn from_sixteenths(sixteenths: u32) -> Self {
        Self(sixteenths)
    }

    pub const fn sixteenths(self) -> u32 {
        self.0
    }

    /// The point `progress` thousandths of the way from `self` to `to`.
    fn toward(self, to: Self, progress: u32) -> Self {
        let (from, to) = (u64::from(self.0), u64::from(to.0));
        let progress = u64::from(progress.min(WHOLE));
        let between = if to >= from {
            from + (to - from) * progress / u64::from(WHOLE)
        } else {
            from - (from - to) * progress / u64::from(WHOLE)
        };
        Self::from_sixteenths(u32::try_from(between).unwrap_or(u32::MAX))
    }
}

impl From<Sets> for Fill {
    fn from(sets: Sets) -> Self {
        Self(sets.halves().saturating_mul(Self::PER_SET / 2))
    }
}

/// A bar growing or shrinking from `from` to the muscle's current sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BarMove {
    pub from: Fill,
    pub tween: Tween,
}

impl BarMove {
    /// Where the bar is drawn at `now` on its way to `to`.
    pub fn at(self, to: Fill, now: Instant) -> Fill {
        self.from.toward(to, self.tween.eased(now))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eased_progress_runs_from_zero_to_whole() {
        let start = Instant::now();
        let tween = Tween::new(start, Duration::from_millis(200));
        assert_eq!(tween.eased(start), 0);
        assert_eq!(tween.eased(start + Duration::from_millis(200)), WHOLE);
        assert_eq!(tween.eased(start + Duration::from_secs(9)), WHOLE);
        let half = tween.eased(start + Duration::from_millis(100));
        assert!(half > 500, "ease-out is past halfway at half time: {half}");
        assert!(tween.is_running(start + Duration::from_millis(199)));
        assert!(!tween.is_running(start + Duration::from_millis(200)));
    }

    #[test]
    fn fill_moves_both_ways() {
        let low = Fill::from(Sets::whole(2));
        let high = Fill::from(Sets::whole(4));
        assert_eq!(low.sixteenths(), 32);
        assert_eq!(low.toward(high, 500).sixteenths(), 48);
        assert_eq!(high.toward(low, 500).sixteenths(), 48);
        assert_eq!(high.toward(low, WHOLE), low);
    }
}
