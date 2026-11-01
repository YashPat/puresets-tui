//! Application state and key handling. Nothing here touches the terminal or
//! the disk: `main` feeds in events and the clock, then saves when
//! [`App::take_dirty`] says the data changed.

mod log_sheet;
mod motion;
mod text_input;

use std::time::{Duration, Instant};

use jiff::civil::Date;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub use log_sheet::{CreateForm, Field, LogSheet, MAX_SETS, NoteEditor, PickRow, SetCount, Step};
pub use motion::{Fill, Motion};
pub use text_input::TextInput;

use crate::domain::{
    Change, Direction, Goal, Muscle, Period, Sets, Target, Tracker, Week, WeekSets,
};
use log_sheet::SheetEvent;
use motion::{BarMove, Tween};

const TOAST_DURATION: Duration = Duration::from_secs(4);
/// How long a new toast takes to slide in.
const TOAST_REVEAL: Duration = Duration::from_millis(150);
/// A toast dims for this long before it goes.
const TOAST_FADE: Duration = Duration::from_millis(600);
const UNDO_LIMIT: usize = 20;
/// How long a Home bar takes to reach a new set count.
const BAR_DURATION: Duration = Duration::from_millis(220);
/// How long a row flashes when its muscle reaches the weekly target.
const PULSE_DURATION: Duration = Duration::from_millis(350);
/// Lines the Why text moves per scroll-wheel step.
const WHEEL_LINES: u16 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Home,
    Insights,
    Why,
    Settings,
}

impl Tab {
    pub const ALL: [Self; 4] = [Self::Home, Self::Insights, Self::Why, Self::Settings];

    pub const fn title(self) -> &'static str {
        match self {
            Self::Home => "Home",
            Self::Insights => "Insights",
            Self::Why => "Why",
            Self::Settings => "Settings",
        }
    }

    fn index(self) -> usize {
        Self::ALL.iter().position(|&tab| tab == self).unwrap_or(0)
    }

    fn offset(self, step: usize) -> Self {
        Self::ALL[(self.index() + step) % Self::ALL.len()]
    }
}

/// What a mouse event landed on, as the last frame laid it out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseTarget {
    /// Something that does exactly what this key does, such as a footer hint.
    Key(KeyEvent),
    Tab(Tab),
    /// The current screen's content; the wheel scrolls or moves through it.
    Body,
    Muscle(Muscle),
    Period(Period),
    /// Outside the open overlay.
    Backdrop,
    /// Inside the open overlay, but on nothing clickable.
    Modal,
    Goal(Goal),
    /// A row of the log sheet's exercise list, by its index in [`LogSheet::rows`].
    LogRow(usize),
    /// A muscle in the create form's grid.
    CreateMuscle(Muscle),
}

impl MouseTarget {
    /// Whether a click here does something, so hovering it is worth showing.
    pub const fn is_clickable(self) -> bool {
        !matches!(self, Self::Body | Self::Backdrop | Self::Modal)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseAction {
    Click,
    ScrollUp,
    ScrollDown,
}

/// A modal layer drawn over the current tab; it receives all keys.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Overlay {
    /// First launch: choose a weekly target.
    Onboarding {
        choice: Goal,
    },
    Help,
    Log(Box<LogSheet>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Success,
    Error,
}

/// A short message in the footer that disappears on its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Toast {
    pub message: String,
    pub kind: ToastKind,
    expires: Instant,
    /// The slide-in, unless motion is off.
    reveal: Option<Tween>,
}

/// How a toast is drawn at one instant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToastFrame<'toast> {
    /// The part of the message shown so far, from its start.
    pub text: &'toast str,
    pub kind: ToastKind,
    /// Whether the toast is about to go and should be drawn dim.
    pub fading: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UndoEntry {
    week: Week,
    before: WeekSets,
    label: String,
}

#[derive(Debug)]
pub struct App {
    pub tracker: Tracker,
    pub tab: Tab,
    pub overlay: Option<Overlay>,
    /// The week shown and edited on the home screen.
    pub week: Week,
    pub selected: Muscle,
    /// First visible line of the home screen when it has to scroll.
    pub home_scroll: usize,
    pub period: Period,
    pub why_scroll: u16,
    pub toast: Option<Toast>,
    /// The clickable thing under the pointer; see [`App::hover`].
    pub hovered: Option<MouseTarget>,
    pub data_path: String,
    pub motion: Motion,
    today: Date,
    now: Instant,
    undo: Vec<UndoEntry>,
    /// Bars still moving to a new count, by the week and muscle they show.
    bars: Vec<(Week, Muscle, BarMove)>,
    /// Muscles that just reached the target, flashing on Home.
    pulses: Vec<(Week, Muscle, Tween)>,
    dirty: bool,
    quit: bool,
}

impl App {
    pub fn new(tracker: Tracker, today: Date, now: Instant, data_path: String) -> Self {
        let overlay = tracker.target.is_none().then_some(Overlay::Onboarding {
            choice: Goal::Moderate,
        });
        Self {
            tracker,
            tab: Tab::Home,
            overlay,
            week: Week::containing(today),
            selected: Muscle::Chest,
            home_scroll: 0,
            period: Period::ThisWeek,
            why_scroll: 0,
            toast: None,
            hovered: None,
            data_path,
            motion: Motion::default(),
            today,
            now,
            undo: Vec::new(),
            bars: Vec::new(),
            pulses: Vec::new(),
            dirty: false,
            quit: false,
        }
    }

    pub fn current_week(&self) -> Week {
        Week::containing(self.today)
    }

    pub fn target(&self) -> Target {
        self.tracker.target_or_default()
    }

    /// The oldest week the home screen can show.
    pub fn earliest_week(&self) -> Week {
        [self.tracker.first_week, self.tracker.log.earliest_week()]
            .into_iter()
            .flatten()
            .chain([self.current_week()])
            .min()
            .unwrap_or_else(|| self.current_week())
    }

    pub fn should_quit(&self) -> bool {
        self.quit
    }

    /// Returns whether the data changed since the last call.
    pub fn take_dirty(&mut self) -> bool {
        std::mem::take(&mut self.dirty)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    /// Whether something is mid-transition, so frames should come quickly.
    pub fn is_animating(&self) -> bool {
        let now = self.now;
        self.bars.iter().any(|(.., bar)| bar.tween.is_running(now))
            || self.pulses.iter().any(|(.., pulse)| pulse.is_running(now))
            || self
                .toast
                .as_ref()
                .and_then(|toast| toast.reveal)
                .is_some_and(|reveal| reveal.is_running(now))
    }

    /// Whether a muscle's row on Home is flashing for reaching the target.
    pub fn is_pulsing(&self, muscle: Muscle) -> bool {
        self.pulses
            .iter()
            .any(|&(week, m, pulse)| week == self.week && m == muscle && pulse.is_running(self.now))
    }

    /// How full a muscle's bar on Home is drawn right now.
    pub fn bar_fill(&self, muscle: Muscle) -> Fill {
        let current = Fill::from(self.tracker.log.sets(self.week, muscle));
        self.bars
            .iter()
            .find(|&&(week, m, _)| week == self.week && m == muscle)
            .map_or(current, |(.., bar)| bar.at(current, self.now))
    }

    /// Advances the clock: ends transitions, expires toasts and follows the
    /// calendar into a new week.
    pub fn tick(&mut self, now: Instant, today: Date) {
        self.now = now;
        self.bars.retain(|(.., bar)| bar.tween.is_running(now));
        self.pulses.retain(|(.., pulse)| pulse.is_running(now));
        if self
            .toast
            .as_ref()
            .is_some_and(|toast| now >= toast.expires)
        {
            self.toast = None;
        }
        if today != self.today {
            let was_current = self.week == self.current_week();
            self.today = today;
            if was_current {
                self.week = self.current_week();
            }
        }
    }

    pub fn notify(&mut self, kind: ToastKind, message: impl Into<String>) {
        let reveal = (self.motion == Motion::On).then(|| Tween::new(self.now, TOAST_REVEAL));
        self.toast = Some(Toast {
            message: message.into(),
            kind,
            expires: self.now + TOAST_DURATION,
            reveal,
        });
    }

    /// The toast as it should look now: partly slid in, or dimmed near its end.
    pub fn toast_frame(&self) -> Option<ToastFrame<'_>> {
        let toast = self.toast.as_ref()?;
        let chars = toast.message.chars().count();
        let shown = toast.reveal.map_or(chars, |reveal| {
            let progress = usize::try_from(reveal.eased(self.now)).unwrap_or(1000);
            (chars * progress).div_ceil(1000)
        });
        let end = toast
            .message
            .char_indices()
            .nth(shown)
            .map_or(toast.message.len(), |(index, _)| index);
        Some(ToastFrame {
            text: &toast.message[..end],
            kind: toast.kind,
            fading: self.now + TOAST_FADE >= toast.expires,
        })
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        if ctrl && key.code == KeyCode::Char('c') {
            self.quit = true;
            return;
        }
        if let Some(overlay) = self.overlay.take() {
            self.overlay = self.overlay_key(overlay, key);
            return;
        }
        match key.code {
            KeyCode::Char('q') => self.quit = true,
            KeyCode::Char('?') => self.overlay = Some(Overlay::Help),
            KeyCode::Tab => self.tab = self.tab.offset(1),
            KeyCode::BackTab => self.tab = self.tab.offset(Tab::ALL.len() - 1),
            KeyCode::Char(c @ '1'..='4') => {
                let index = c
                    .to_digit(10)
                    .and_then(|d| usize::try_from(d).ok())
                    .unwrap_or(1);
                self.tab = Tab::ALL[index - 1];
            }
            _ => match self.tab {
                Tab::Home => self.home_key(key),
                Tab::Insights => self.insights_key(key),
                Tab::Why => self.why_key(key),
                Tab::Settings => self.settings_key(key),
            },
        }
    }

    /// Records what the pointer is over; returns whether that changed, so
    /// moves within one target need no redraw.
    pub fn hover(&mut self, target: Option<MouseTarget>) -> bool {
        let target = target.filter(|target| target.is_clickable());
        let changed = self.hovered != target;
        self.hovered = target;
        changed
    }

    /// The mouse is an extra: every action here has a key that does the same.
    pub fn handle_mouse(&mut self, target: MouseTarget, action: MouseAction) {
        if let (MouseTarget::Key(key), MouseAction::Click) = (target, action) {
            self.handle_key(key);
            return;
        }
        if let Some(overlay) = self.overlay.take() {
            self.overlay = self.overlay_mouse(overlay, target, action);
            return;
        }
        match (target, action) {
            (MouseTarget::Tab(tab), MouseAction::Click) => self.tab = tab,
            // A second click on the selected muscle logs it, as enter does.
            (MouseTarget::Muscle(muscle), MouseAction::Click) if self.tab == Tab::Home => {
                if self.selected == muscle {
                    self.open_log(Some(muscle));
                } else {
                    self.selected = muscle;
                }
            }
            (MouseTarget::Period(period), MouseAction::Click) if self.tab == Tab::Insights => {
                self.period = period;
            }
            (MouseTarget::Body | MouseTarget::Muscle(_), MouseAction::ScrollUp) => {
                self.wheel(KeyCode::Up);
            }
            (MouseTarget::Body | MouseTarget::Muscle(_), MouseAction::ScrollDown) => {
                self.wheel(KeyCode::Down);
            }
            _ => {}
        }
    }

    /// The wheel moves the home selection one muscle and the Why text a few lines.
    fn wheel(&mut self, code: KeyCode) {
        match (self.tab, code) {
            (Tab::Home, _) => self.home_key(KeyEvent::from(code)),
            (Tab::Why, KeyCode::Up) => {
                self.why_scroll = self.why_scroll.saturating_sub(WHEEL_LINES);
            }
            (Tab::Why, _) => self.why_scroll = self.why_scroll.saturating_add(WHEEL_LINES),
            (Tab::Insights | Tab::Settings, _) => {}
        }
    }

    fn overlay_mouse(
        &mut self,
        overlay: Overlay,
        target: MouseTarget,
        action: MouseAction,
    ) -> Option<Overlay> {
        match (overlay, target, action) {
            (Overlay::Help, MouseTarget::Backdrop, MouseAction::Click) => None,
            // Click a goal to choose it, then again to confirm.
            (Overlay::Onboarding { choice }, MouseTarget::Goal(goal), MouseAction::Click) => {
                if goal == choice {
                    self.overlay_key(
                        Overlay::Onboarding { choice },
                        KeyEvent::from(KeyCode::Enter),
                    )
                } else {
                    Some(Overlay::Onboarding { choice: goal })
                }
            }
            // Clicking away only closes the sheet while nothing is half done.
            (Overlay::Log(sheet), MouseTarget::Backdrop, MouseAction::Click)
                if sheet.step == Step::Pick =>
            {
                None
            }
            (Overlay::Log(mut sheet), target, action) => {
                let event = sheet.handle_mouse(target, action, &mut self.tracker);
                self.sheet_event(sheet, event)
            }
            (overlay, ..) => Some(overlay),
        }
    }

    pub fn paste(&mut self, text: &str) {
        if let Some(Overlay::Log(sheet)) = &mut self.overlay {
            sheet.paste(text);
        }
    }

    /// Opens the log sheet, optionally filtered to one muscle.
    pub fn open_log(&mut self, filter: Option<Muscle>) {
        self.overlay = Some(Overlay::Log(Box::new(LogSheet::new(filter))));
    }

    fn overlay_key(&mut self, overlay: Overlay, key: KeyEvent) -> Option<Overlay> {
        match overlay {
            Overlay::Help => match key.code {
                KeyCode::Esc | KeyCode::Enter | KeyCode::Char('?' | 'q') => None,
                _ => Some(Overlay::Help),
            },
            Overlay::Onboarding { choice } => match key.code {
                KeyCode::Up | KeyCode::Down | KeyCode::Char('j' | 'k') | KeyCode::Tab => {
                    let choice = match choice {
                        Goal::Moderate => Goal::Optimal,
                        Goal::Optimal => Goal::Moderate,
                    };
                    Some(Overlay::Onboarding { choice })
                }
                KeyCode::Enter => {
                    self.set_target(choice.target());
                    self.notify(
                        ToastKind::Success,
                        format!(
                            "Target: {} sets per muscle. Press enter to log sets.",
                            choice.target()
                        ),
                    );
                    None
                }
                KeyCode::Char('q') => {
                    self.quit = true;
                    None
                }
                _ => Some(Overlay::Onboarding { choice }),
            },
            Overlay::Log(mut sheet) => {
                let event = sheet.handle_key(key, &mut self.tracker);
                self.sheet_event(sheet, event)
            }
        }
    }

    /// Acts on what the log sheet asked for; returns the sheet unless it closed.
    fn sheet_event(&mut self, sheet: Box<LogSheet>, event: SheetEvent) -> Option<Overlay> {
        match event {
            SheetEvent::None => Some(Overlay::Log(sheet)),
            SheetEvent::Close => None,
            SheetEvent::Log { exercise, count } => {
                self.log_exercise(&exercise, count);
                None
            }
            SheetEvent::Saved(message) => {
                self.dirty = true;
                self.notify(ToastKind::Success, message);
                Some(Overlay::Log(sheet))
            }
            SheetEvent::Notice(message) => {
                self.notify(ToastKind::Info, message);
                Some(Overlay::Log(sheet))
            }
        }
    }

    fn home_key(&mut self, key: KeyEvent) {
        let index = self.selected.index();
        match key.code {
            KeyCode::Down | KeyCode::Char('j') => {
                self.selected = Muscle::ALL[(index + 1).min(Muscle::ALL.len() - 1)];
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.selected = Muscle::ALL[index.saturating_sub(1)];
            }
            KeyCode::Home | KeyCode::Char('g') => self.selected = Muscle::ALL[0],
            KeyCode::End | KeyCode::Char('G') => self.selected = Muscle::ALL[Muscle::ALL.len() - 1],
            KeyCode::Left | KeyCode::Char('h') => self.show_week(self.week.previous()),
            KeyCode::Right | KeyCode::Char('l') => self.show_week(self.week.next()),
            KeyCode::Char('t') => self.week = self.current_week(),
            KeyCode::Enter => self.open_log(Some(self.selected)),
            KeyCode::Char('a' | '/') => self.open_log(None),
            KeyCode::Char('+' | '=') => self.adjust(Change::Add(Sets::ONE)),
            KeyCode::Char('-' | '_') => self.adjust(Change::Remove(Sets::ONE)),
            KeyCode::Char(']') => self.adjust(Change::Add(Sets::HALF)),
            KeyCode::Char('[') => self.adjust(Change::Remove(Sets::HALF)),
            KeyCode::Char('u') => self.undo(),
            _ => {}
        }
    }

    fn insights_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Left | KeyCode::Char('h') => self.period = self.period.previous(),
            KeyCode::Right | KeyCode::Char('l') => self.period = self.period.next(),
            _ => {}
        }
    }

    fn why_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Down | KeyCode::Char('j') => {
                self.why_scroll = self.why_scroll.saturating_add(1);
            }
            KeyCode::Up | KeyCode::Char('k') => self.why_scroll = self.why_scroll.saturating_sub(1),
            KeyCode::PageDown | KeyCode::Char(' ') => {
                self.why_scroll = self.why_scroll.saturating_add(10);
            }
            KeyCode::PageUp => self.why_scroll = self.why_scroll.saturating_sub(10),
            KeyCode::Home | KeyCode::Char('g') => self.why_scroll = 0,
            _ => {}
        }
    }

    fn settings_key(&mut self, key: KeyEvent) {
        let target = self.target();
        match key.code {
            KeyCode::Left | KeyCode::Char('h' | '-') => {
                self.set_target(target.step(Direction::Down));
            }
            KeyCode::Right | KeyCode::Char('l' | '+' | '=') => {
                self.set_target(target.step(Direction::Up));
            }
            KeyCode::Char('m') => self.set_target(Goal::Moderate.target()),
            KeyCode::Char('o') => self.set_target(Goal::Optimal.target()),
            _ => {}
        }
    }

    fn set_target(&mut self, target: Target) {
        if self.tracker.target != Some(target) {
            self.tracker.target = Some(target);
            self.dirty = true;
        }
        if self.tracker.first_week.is_none() {
            self.tracker.first_week = Some(self.current_week());
            self.dirty = true;
        }
    }

    fn show_week(&mut self, week: Week) {
        if week > self.current_week() {
            self.notify(ToastKind::Info, "That's the current week.");
        } else if week < self.earliest_week() {
            self.notify(ToastKind::Info, "That's the first week you tracked.");
        } else {
            self.week = week;
        }
    }

    fn log_exercise(&mut self, name: &str, count: u32) {
        let before = self.tracker.log.week(self.week);
        let Some(credits) = self.tracker.log_sets(self.week, name, count) else {
            self.notify(ToastKind::Error, format!("No exercise named '{name}'."));
            return;
        };
        let canonical = self
            .tracker
            .catalog
            .get(name)
            .map_or(name, |exercise| exercise.name())
            .to_owned();
        let label = format!("{count} × {canonical}");
        let summary: Vec<String> = credits
            .iter()
            .map(|credit| format!("{} +{}", credit.muscle, credit.sets))
            .collect();
        self.notify(
            ToastKind::Success,
            format!("Logged {label}  ·  {}  ·  u to undo", summary.join(", ")),
        );
        self.animate_change(self.week, &before);
        self.push_undo(before, label);
    }

    fn adjust(&mut self, change: Change) {
        let muscle = self.selected;
        let before_sets = self.tracker.log.sets(self.week, muscle);
        let before = self.tracker.log.week(self.week);
        self.tracker.log.adjust(self.week, muscle, change);
        let after = self.tracker.log.sets(self.week, muscle);
        if after == before_sets {
            return;
        }
        self.animate_change(self.week, &before);
        self.push_undo(before, format!("{muscle} {before_sets} → {after}"));
    }

    /// Starts a bar move for every muscle whose sets in `week` differ from
    /// `before`. A bar already moving starts again from where it is drawn.
    fn animate_change(&mut self, week: Week, before: &WeekSets) {
        if self.motion == Motion::Off {
            return;
        }
        let now = self.now;
        let goal = self.target().sets();
        for muscle in Muscle::ALL {
            let old = before.get(&muscle).copied().unwrap_or_default();
            let new = self.tracker.log.sets(week, muscle);
            if new == old {
                continue;
            }
            if old < goal && goal <= new {
                self.pulses.retain(|&(w, m, _)| (w, m) != (week, muscle));
                self.pulses
                    .push((week, muscle, Tween::new(now, PULSE_DURATION)));
            }
            let mut from = Fill::from(old);
            if let Some(index) = self
                .bars
                .iter()
                .position(|&(w, m, _)| w == week && m == muscle)
            {
                let (.., bar) = self.bars.swap_remove(index);
                from = bar.at(from, now);
            }
            let tween = Tween::new(now, BAR_DURATION);
            self.bars.push((week, muscle, BarMove { from, tween }));
        }
    }

    fn push_undo(&mut self, before: WeekSets, label: String) {
        self.undo.push(UndoEntry {
            week: self.week,
            before,
            label,
        });
        if self.undo.len() > UNDO_LIMIT {
            self.undo.remove(0);
        }
        self.dirty = true;
    }

    fn undo(&mut self) {
        let Some(entry) = self.undo.pop() else {
            self.notify(ToastKind::Info, "Nothing to undo.");
            return;
        };
        let undone = self.tracker.log.week(entry.week);
        self.tracker.log.restore(entry.week, entry.before);
        self.animate_change(entry.week, &undone);
        self.week = entry.week;
        self.dirty = true;
        self.notify(ToastKind::Info, format!("Undid {}.", entry.label));
    }
}

#[cfg(test)]
mod tests {
    use jiff::civil::date;

    use super::*;
    use crate::domain::Catalog;

    fn app() -> App {
        let tracker = Tracker::new(Catalog::built_in().unwrap());
        App::new(
            tracker,
            date(2026, 11, 1),
            Instant::now(),
            "data.json".into(),
        )
    }

    fn onboarded() -> App {
        let mut app = app();
        press(&mut app, &[KeyCode::Enter]);
        app
    }

    fn press(app: &mut App, keys: &[KeyCode]) {
        for &key in keys {
            app.handle_key(KeyEvent::from(key));
        }
    }

    fn type_text(app: &mut App, text: &str) {
        for c in text.chars() {
            app.handle_key(KeyEvent::from(KeyCode::Char(c)));
        }
    }

    fn sets(app: &App, muscle: Muscle) -> Sets {
        app.tracker.log.sets(app.week, muscle)
    }

    #[test]
    fn first_launch_asks_for_a_goal() {
        let mut app = app();
        assert!(matches!(app.overlay, Some(Overlay::Onboarding { .. })));
        press(&mut app, &[KeyCode::Down, KeyCode::Enter]);
        assert_eq!(app.tracker.target, Some(Goal::Optimal.target()));
        assert_eq!(app.tracker.first_week, Some(app.current_week()));
        assert!(app.overlay.is_none());
        assert!(app.take_dirty());
    }

    #[test]
    fn logs_an_exercise_by_searching() {
        let mut app = onboarded();
        press(&mut app, &[KeyCode::Char('a')]);
        type_text(&mut app, "incline");
        press(&mut app, &[KeyCode::Enter]);
        type_text(&mut app, "3");
        press(&mut app, &[KeyCode::Enter]);

        assert!(app.overlay.is_none());
        assert_eq!(sets(&app, Muscle::Chest), Sets::whole(3));
        assert_eq!(sets(&app, Muscle::Triceps), Sets::from_halves(3));
        let toast = app.toast.as_ref().unwrap();
        assert!(
            toast.message.starts_with("Logged 3 × Incline Bench Press"),
            "{}",
            toast.message
        );
    }

    #[test]
    fn enter_on_home_filters_by_the_selected_muscle() {
        let mut app = onboarded();
        press(&mut app, &[KeyCode::Char('j'), KeyCode::Enter]);
        let Some(Overlay::Log(sheet)) = &app.overlay else {
            panic!("log sheet should be open");
        };
        assert_eq!(sheet.filter, Some(Muscle::Triceps));
        assert!(sheet.rows(&app.tracker).iter().all(|row| match row {
            PickRow::Exercise { exercise, .. } => exercise.works(Muscle::Triceps),
            PickRow::NewCustom => true,
        }));
    }

    #[test]
    fn set_count_is_bounded_and_typed() {
        let mut app = onboarded();
        press(&mut app, &[KeyCode::Char('a'), KeyCode::Enter]);
        let count = |app: &App| match &app.overlay {
            Some(Overlay::Log(sheet)) => match &sheet.step {
                Step::Sets(sets) => sets.count,
                other => panic!("unexpected step {other:?}"),
            },
            other => panic!("unexpected overlay {other:?}"),
        };
        press(&mut app, &[KeyCode::Down]);
        assert_eq!(count(&app), 1);
        type_text(&mut app, "12");
        assert_eq!(count(&app), 12);
        type_text(&mut app, "5");
        assert_eq!(
            count(&app),
            5,
            "125 is over the limit, so 5 starts a new number"
        );
        type_text(&mut app, "0");
        assert_eq!(count(&app), 5, "50 is over the limit");
        for _ in 0..30 {
            press(&mut app, &[KeyCode::Up]);
        }
        assert_eq!(count(&app), 20);
    }

    #[test]
    fn escape_clears_the_query_before_closing() {
        let mut app = onboarded();
        press(&mut app, &[KeyCode::Char('a')]);
        type_text(&mut app, "q");
        assert!(!app.should_quit(), "q is text while searching");
        press(&mut app, &[KeyCode::Esc]);
        assert!(app.overlay.is_some());
        press(&mut app, &[KeyCode::Esc]);
        assert!(app.overlay.is_none());
    }

    #[test]
    fn adjust_and_undo() {
        let mut app = onboarded();
        press(
            &mut app,
            &[KeyCode::Char('+'), KeyCode::Char('+'), KeyCode::Char('[')],
        );
        assert_eq!(sets(&app, Muscle::Chest), Sets::from_halves(3));
        press(&mut app, &[KeyCode::Char('u')]);
        assert_eq!(sets(&app, Muscle::Chest), Sets::whole(2));
        press(&mut app, &[KeyCode::Char('u'), KeyCode::Char('u')]);
        assert_eq!(sets(&app, Muscle::Chest), Sets::ZERO);
        assert!(!app.can_undo());
        press(&mut app, &[KeyCode::Char('-')]);
        assert!(!app.can_undo(), "removing from zero changes nothing");
    }

    #[test]
    fn week_navigation_is_bounded() {
        let mut app = onboarded();
        let current = app.current_week();
        press(&mut app, &[KeyCode::Right]);
        assert_eq!(app.week, current);
        press(&mut app, &[KeyCode::Left]);
        assert_eq!(app.week, current, "nothing tracked before this week");

        app.tracker.log.adjust(
            current.previous().previous(),
            Muscle::Abs,
            Change::Add(Sets::ONE),
        );
        press(&mut app, &[KeyCode::Left, KeyCode::Left, KeyCode::Left]);
        assert_eq!(app.week, current.previous().previous());
        press(&mut app, &[KeyCode::Char('+')]);
        assert_eq!(
            sets(&app, Muscle::Chest),
            Sets::ONE,
            "edits apply to the shown week"
        );
        press(&mut app, &[KeyCode::Char('t')]);
        assert_eq!(app.week, current);
    }

    #[test]
    fn creates_a_custom_exercise() {
        let mut app = onboarded();
        press(&mut app, &[KeyCode::Char('a')]);
        type_text(&mut app, "Cable Crossover");
        press(&mut app, &[KeyCode::Enter]); // no matches: opens the form
        press(&mut app, &[KeyCode::Enter]);
        let error = match &app.overlay {
            Some(Overlay::Log(sheet)) => match &sheet.step {
                Step::Create(form) => form.error.clone(),
                other => panic!("unexpected step {other:?}"),
            },
            other => panic!("unexpected overlay {other:?}"),
        };
        assert_eq!(
            error.as_deref(),
            Some("At least one primary muscle is required.")
        );

        press(&mut app, &[KeyCode::Tab, KeyCode::Char(' ')]); // chest → primary
        press(
            &mut app,
            &[KeyCode::Char('j'), KeyCode::Char('j'), KeyCode::Char('s')],
        ); // front delts
        press(&mut app, &[KeyCode::Enter]);
        assert!(app.take_dirty());
        let exercise = app.tracker.catalog.get("cable crossover").unwrap();
        assert_eq!(exercise.primary(), [Muscle::Chest]);
        assert_eq!(exercise.secondary(), [Muscle::FrontDelts]);

        press(&mut app, &[KeyCode::Char('2'), KeyCode::Enter]);
        assert_eq!(sets(&app, Muscle::Chest), Sets::whole(2));
        assert_eq!(sets(&app, Muscle::FrontDelts), Sets::ONE);
    }

    #[test]
    fn deletes_only_custom_exercises() {
        let mut app = onboarded();
        press(&mut app, &[KeyCode::Char('a')]);
        type_text(&mut app, "bench press");
        let ctrl_d = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL);
        app.handle_key(ctrl_d);
        assert_eq!(
            app.toast.as_ref().map(|t| t.message.as_str()),
            Some("Built-in exercises cannot be deleted.")
        );

        app.tracker
            .catalog
            .add_custom(crate::domain::ExerciseDraft {
                name: "Zercher Squat".into(),
                primary: vec![Muscle::Quads],
                ..Default::default()
            })
            .unwrap();
        press(&mut app, &[KeyCode::Esc]);
        type_text(&mut app, "zercher");
        app.handle_key(ctrl_d);
        press(&mut app, &[KeyCode::Char('y')]);
        assert!(app.tracker.catalog.get("Zercher Squat").is_none());
    }

    #[test]
    fn notes_are_edited_from_the_set_step() {
        let mut app = onboarded();
        press(&mut app, &[KeyCode::Char('a')]);
        type_text(&mut app, "squat");
        press(
            &mut app,
            &[KeyCode::Down, KeyCode::Enter, KeyCode::Char('n')],
        );
        app.paste("Belt from 140");
        press(&mut app, &[KeyCode::Enter]);
        let name = match &app.overlay {
            Some(Overlay::Log(sheet)) => match &sheet.step {
                Step::Sets(sets) => sets.exercise.clone(),
                other => panic!("unexpected step {other:?}"),
            },
            other => panic!("unexpected overlay {other:?}"),
        };
        assert_eq!(
            app.tracker.catalog.get(&name).unwrap().note(),
            "Belt from 140"
        );
    }

    #[test]
    fn settings_change_the_target() {
        let mut app = onboarded();
        press(
            &mut app,
            &[KeyCode::Char('4'), KeyCode::Right, KeyCode::Right],
        );
        assert_eq!(app.target(), Target::new(10).unwrap());
        press(&mut app, &[KeyCode::Char('o')]);
        assert_eq!(app.target(), Goal::Optimal.target());
    }

    #[test]
    fn bars_move_to_a_new_count_over_time() {
        let mut app = onboarded();
        let start = app.now;
        let today = date(2026, 11, 1);
        press(&mut app, &[KeyCode::Char('+')]);
        assert!(app.is_animating());
        assert_eq!(app.bar_fill(Muscle::Chest), Fill::default(), "starts empty");
        assert_eq!(
            app.bar_fill(Muscle::Triceps),
            Fill::default(),
            "only the changed muscle moves"
        );

        app.tick(start + BAR_DURATION / 2, today);
        let middle = app.bar_fill(Muscle::Chest);
        assert!(
            Fill::default() < middle && middle < Fill::from(Sets::ONE),
            "halfway the bar is part filled: {middle:?}"
        );

        // A second change mid-move starts from where the bar is drawn.
        press(&mut app, &[KeyCode::Char('+')]);
        assert_eq!(app.bar_fill(Muscle::Chest), middle);

        app.tick(start + BAR_DURATION * 2, today);
        assert!(!app.is_animating());
        assert_eq!(app.bar_fill(Muscle::Chest), Fill::from(Sets::whole(2)));

        press(&mut app, &[KeyCode::Char('u')]);
        assert!(app.is_animating(), "undo moves the bar back too");
    }

    #[test]
    fn logging_moves_every_credited_bar_unless_motion_is_off() {
        let mut app = onboarded();
        app.open_log(None);
        type_text(&mut app, "bench press");
        press(&mut app, &[KeyCode::Enter, KeyCode::Enter]);
        assert!(sets(&app, Muscle::Triceps) > Sets::ZERO);
        assert_eq!(app.bar_fill(Muscle::Chest), Fill::default());
        assert_eq!(app.bar_fill(Muscle::Triceps), Fill::default());

        let mut still = onboarded();
        still.motion = Motion::Off;
        still.toast = None;
        press(&mut still, &[KeyCode::Char('+')]);
        assert!(!still.is_animating());
        assert_eq!(still.bar_fill(Muscle::Chest), Fill::from(Sets::ONE));
    }

    #[test]
    fn reaching_the_target_flashes_the_row_once() {
        let mut app = onboarded();
        let start = app.now;
        let goal = app.target().sets().halves();
        for _ in 1..goal / 2 {
            press(&mut app, &[KeyCode::Char('+')]);
        }
        assert!(!app.is_pulsing(Muscle::Chest), "still below the target");

        press(&mut app, &[KeyCode::Char('+')]);
        assert_eq!(sets(&app, Muscle::Chest), app.target().sets());
        assert!(app.is_pulsing(Muscle::Chest));
        assert!(!app.is_pulsing(Muscle::UpperBack));

        app.tick(start + PULSE_DURATION, date(2026, 11, 1));
        assert!(!app.is_pulsing(Muscle::Chest));
        press(&mut app, &[KeyCode::Char('+')]);
        assert!(!app.is_pulsing(Muscle::Chest), "already past the target");
    }

    #[test]
    fn toasts_slide_in_then_dim_before_they_go() {
        let mut app = onboarded();
        let start = app.now;
        let today = date(2026, 11, 1);
        press(&mut app, &[KeyCode::Char('u')]);
        let message = app.toast.as_ref().map(|t| t.message.clone()).unwrap();
        let frame = app.toast_frame().unwrap();
        assert_eq!(frame.text, "", "nothing shows before the slide starts");
        assert!(app.is_animating());

        app.tick(start + TOAST_REVEAL / 3, today);
        let frame = app.toast_frame().unwrap();
        assert!(
            !frame.text.is_empty() && frame.text.len() < message.len(),
            "part way in: {:?}",
            frame.text
        );
        assert!(message.starts_with(frame.text));

        app.tick(start + TOAST_REVEAL, today);
        assert!(!app.is_animating());
        let frame = app.toast_frame().unwrap();
        assert_eq!(frame.text, message);
        assert!(!frame.fading);

        let fade_start = (start + TOAST_DURATION)
            .checked_sub(TOAST_FADE)
            .expect("the clock is past the fade length");
        app.tick(fade_start, today);
        assert!(app.toast_frame().unwrap().fading);
    }

    #[test]
    fn toasts_show_whole_with_motion_off() {
        let mut app = onboarded();
        app.motion = Motion::Off;
        press(&mut app, &[KeyCode::Char('u')]);
        assert_eq!(
            app.toast_frame().map(|frame| frame.text),
            Some("Nothing to undo.")
        );
        assert!(!app.is_animating());
    }

    #[test]
    fn toasts_expire_and_the_week_rolls_over() {
        let mut app = onboarded();
        let start = app.now;
        assert!(app.toast.is_some());
        app.tick(start + TOAST_DURATION, date(2026, 11, 2));
        assert!(app.toast.is_none());
        assert!(app.toast_frame().is_none());
        assert_eq!(app.week.to_string(), "2026-W45");
    }
}
