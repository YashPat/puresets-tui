//! Snapshot tests: every tab and every log-sheet step, drawn at fixed sizes
//! from a seeded tracker on a fixed date. Review changes with `cargo insta review`.

use std::time::Instant;

use jiff::civil::date;
use ratatui::{
    Terminal,
    backend::TestBackend,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
};

use super::{Theme, draw};
use crate::{
    app::{App, Tab},
    domain::{Catalog, ExerciseDraft, Muscle, Target, Tracker, Week},
};

const SIZES: [(u16, u16); 2] = [(80, 24), (120, 40)];

/// A tracker with five weeks of training, a custom exercise and a note.
fn seeded_tracker() -> Tracker {
    let mut tracker = Tracker::new(Catalog::built_in().expect("bundled catalog parses"));
    tracker.target = Some(Target::new(10).expect("10 is a valid target"));
    tracker
        .catalog
        .add_custom(ExerciseDraft {
            name: "Cable Fly".into(),
            primary: vec![Muscle::Chest],
            secondary: vec![Muscle::FrontDelts],
            note: "low pulley".into(),
        })
        .expect("Cable Fly is a valid custom exercise");
    tracker.catalog.set_note("Squat", "belt from 140");

    let this_week = Week::containing(date(2026, 11, 4));
    let mut week = this_week;
    for _ in 0..4 {
        week = week.previous();
    }
    tracker.first_week = Some(week);
    let session: [(&str, u32); 6] = [
        ("Squat", 4),
        ("Bench Press", 3),
        ("Pull-up", 3),
        ("Romanian Deadlift", 3),
        ("Lateral Raise", 4),
        ("Bicep Curl", 2),
    ];
    for offset in 0..5 {
        for &(exercise, count) in &session {
            // Ramp volume up over the weeks so Insights has a trend to show.
            tracker
                .log_sets(week, exercise, count + offset / 2)
                .expect("seeded exercises exist");
        }
        week = week.next();
    }
    tracker
        .log_sets(this_week, "Cable Fly", 3)
        .expect("Cable Fly was added above");
    tracker
}

fn seeded_app() -> App {
    App::new(
        seeded_tracker(),
        date(2026, 11, 4),
        Instant::now(),
        "~/data.json".into(),
    )
}

fn press(app: &mut App, key: KeyEvent) {
    app.handle_key(key);
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::from(code)
}

fn type_text(app: &mut App, text: &str) {
    for c in text.chars() {
        press(app, key(KeyCode::Char(c)));
    }
}

/// Draws one frame and returns the screen as text, trailing spaces trimmed.
fn render(app: &mut App, width: u16, height: u16) -> String {
    let mut terminal =
        Terminal::new(TestBackend::new(width, height)).expect("test backend never fails");
    terminal
        .draw(|frame| draw(frame, app, Theme::COLOR))
        .expect("drawing to a test backend never fails");
    let buffer = terminal.backend().buffer();
    let mut screen = String::new();
    for y in 0..buffer.area.height {
        let mut line = String::new();
        for x in 0..buffer.area.width {
            line.push_str(buffer[(x, y)].symbol());
        }
        screen.push_str(line.trim_end());
        screen.push('\n');
    }
    screen
}

fn assert_screens(name: &str, mut setup: impl FnMut() -> App) {
    for (width, height) in SIZES {
        let mut app = setup();
        insta::assert_snapshot!(
            format!("{name}_{width}x{height}"),
            render(&mut app, width, height)
        );
    }
}

#[test]
fn tabs() {
    for tab in Tab::ALL {
        assert_screens(&format!("tab_{}", tab.title().to_lowercase()), || {
            let mut app = seeded_app();
            app.tab = tab;
            app
        });
    }
}

/// Opens the log sheet with `a`, searching all exercises.
fn log_sheet() -> App {
    let mut app = seeded_app();
    type_text(&mut app, "a");
    app
}

#[test]
fn log_sheet_pick() {
    assert_screens("log_pick", log_sheet);
}

#[test]
fn log_sheet_pick_search() {
    assert_screens("log_pick_search", || {
        let mut app = log_sheet();
        type_text(&mut app, "press");
        app
    });
}

#[test]
fn log_sheet_sets() {
    assert_screens("log_sets", || {
        let mut app = log_sheet();
        type_text(&mut app, "bench press");
        press(&mut app, key(KeyCode::Enter));
        app
    });
}

#[test]
fn log_sheet_note() {
    assert_screens("log_note", || {
        let mut app = log_sheet();
        type_text(&mut app, "cable fly");
        press(&mut app, key(KeyCode::Enter));
        type_text(&mut app, "n");
        app
    });
}

#[test]
fn log_sheet_create() {
    assert_screens("log_create", || {
        let mut app = log_sheet();
        type_text(&mut app, "zercher squat");
        press(&mut app, key(KeyCode::Enter));
        app
    });
}

#[test]
fn log_sheet_confirm_delete() {
    assert_screens("log_confirm_delete", || {
        let mut app = log_sheet();
        type_text(&mut app, "cable fly");
        press(
            &mut app,
            KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL),
        );
        app
    });
}
