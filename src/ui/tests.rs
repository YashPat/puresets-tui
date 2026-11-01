//! Render tests: draw real app states into an in-memory terminal and check
//! the text that lands on screen.

use std::time::Instant;

use jiff::civil::date;
use ratatui::{
    Terminal,
    backend::TestBackend,
    buffer::Buffer,
    crossterm::event::{KeyCode, KeyEvent},
    style::Modifier,
};

use super::{MIN_HEIGHT, MIN_WIDTH, Theme, draw};
use crate::{
    app::{App, Motion, Tab},
    domain::{Catalog, Tracker},
};

const SIZES: [(u16, u16); 3] = [(MIN_WIDTH, MIN_HEIGHT), (80, 24), (140, 45)];

/// The clock never moves here, so motion is off and frames show end states.
fn app() -> App {
    let tracker = Tracker::new(Catalog::built_in().unwrap());
    let mut app = App::new(
        tracker,
        date(2026, 11, 1),
        Instant::now(),
        "~/data.json".into(),
    );
    app.motion = Motion::Off;
    app
}

fn onboarded() -> App {
    let mut app = app();
    press(&mut app, &[KeyCode::Enter]);
    // Clear the welcome toast so the footer shows key hints.
    app.toast = None;
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

/// Draws one frame and returns the screen as lines of text.
fn render(app: &mut App, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| {
            draw(frame, app, Theme::COLOR);
        })
        .unwrap();
    let buffer = terminal.backend().buffer();
    let mut screen = String::new();
    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width {
            screen.push_str(buffer[(x, y)].symbol());
        }
        screen.push('\n');
    }
    screen
}

#[test]
fn first_launch_asks_for_a_target() {
    let screen = render(&mut app(), 80, 24);
    assert!(screen.contains("Welcome to PureSets"), "{screen}");
    assert!(screen.contains("Moderate"), "{screen}");
    assert!(screen.contains("Optimal"), "{screen}");
}

#[test]
fn every_tab_renders_at_every_size() {
    for tab in Tab::ALL {
        for (width, height) in SIZES {
            let mut app = onboarded();
            app.tab = tab;
            let screen = render(&mut app, width, height);
            assert!(
                screen.contains("PureSets"),
                "{tab:?} at {width}×{height}:\n{screen}"
            );
            assert!(
                screen.contains("? help"),
                "{tab:?} at {width}×{height} has no help hint:\n{screen}"
            );
        }
    }
}

#[test]
fn home_lists_muscles_and_this_week() {
    let screen = render(&mut onboarded(), 80, 24);
    for text in ["Week 44", "this week", "PUSH", "Chest", "0 / 8"] {
        assert!(screen.contains(text), "missing {text:?}:\n{screen}");
    }
}

#[test]
fn home_uses_two_columns_when_wide() {
    let screen = render(&mut onboarded(), 140, 45);
    let row = screen
        .lines()
        .find(|line| line.contains("PUSH"))
        .expect("a PUSH header");
    assert!(
        row.contains("LEGS"),
        "two categories share a row when wide:\n{screen}"
    );
}

#[test]
fn small_terminal_asks_for_more_room() {
    let screen = render(&mut onboarded(), MIN_WIDTH - 1, MIN_HEIGHT);
    assert!(screen.contains("bigger window"), "{screen}");
}

#[test]
fn logging_flow_renders_each_step() {
    let mut app = onboarded();
    type_text(&mut app, "a");
    let screen = render(&mut app, 100, 32);
    assert!(screen.contains("Log sets"), "pick step:\n{screen}");

    type_text(&mut app, "bench press");
    let screen = render(&mut app, 100, 32);
    assert!(screen.contains("Bench Press"), "search results:\n{screen}");

    press(&mut app, &[KeyCode::Enter]);
    let screen = render(&mut app, 100, 32);
    assert!(screen.contains("hard set"), "sets step:\n{screen}");
    assert!(screen.contains("Chest"), "credited muscles:\n{screen}");

    press(&mut app, &[KeyCode::Enter]);
    let screen = render(&mut app, 100, 32);
    assert!(app.overlay.is_none(), "the sheet closes after logging");
    assert!(
        screen.contains("Bench Press"),
        "a toast confirms the log:\n{screen}"
    );
}

#[test]
fn search_with_no_match_offers_to_create() {
    let mut app = onboarded();
    type_text(&mut app, "a");
    type_text(&mut app, "zzz nothing");
    let screen = render(&mut app, 100, 32);
    assert!(screen.contains("Create"), "{screen}");
}

#[test]
fn help_lists_shortcuts_at_both_widths() {
    for (width, height) in [(80, 40), (120, 40)] {
        let mut app = onboarded();
        type_text(&mut app, "?");
        let screen = render(&mut app, width, height);
        assert!(screen.contains("Keyboard"), "{width}×{height}:\n{screen}");
        assert!(screen.contains("Log sheet"), "{width}×{height}:\n{screen}");
    }
}

#[test]
fn mono_theme_renders_without_colour() {
    let mut app = onboarded();
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    terminal
        .draw(|frame| {
            draw(frame, &mut app, Theme::MONO);
        })
        .unwrap();
    let buffer = terminal.backend().buffer();
    assert!(
        buffer
            .content()
            .iter()
            .all(|cell| cell.fg == ratatui::style::Color::Reset),
        "mono uses no foreground colours"
    );
}

/// Draws one frame in `theme` and returns the raw cells.
fn cells(app: &mut App, theme: Theme) -> Buffer {
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    terminal
        .draw(|frame| {
            draw(frame, app, theme);
        })
        .unwrap();
    terminal.backend().buffer().clone()
}

/// Whether the cells where `text` first appears are drawn in reverse video.
fn reversed_at(buffer: &Buffer, text: &str) -> bool {
    let (x, y) = locate(buffer, text);
    buffer[(x, y)].modifier.contains(Modifier::REVERSED)
}

/// The cell where `text` first appears on screen.
fn locate(buffer: &Buffer, text: &str) -> (u16, u16) {
    let width = buffer.area.width;
    let rows: Vec<String> = (0..buffer.area.height)
        .map(|y| (0..width).map(|x| buffer[(x, y)].symbol()).collect())
        .collect();
    let (y, column) = rows
        .iter()
        .enumerate()
        .find_map(|(y, row)| row.find(text).map(|byte| (y, row[..byte].chars().count())))
        .unwrap_or_else(|| panic!("{text:?} is on screen"));
    let x = u16::try_from(column).unwrap();
    let y = u16::try_from(y).unwrap();
    (x, y)
}

#[test]
fn reaching_the_target_flashes_without_colour_too() {
    let mut app = onboarded();
    app.motion = Motion::On;
    let halves = app.target().sets().halves();
    for _ in 0..halves / 2 {
        press(&mut app, &[KeyCode::Char('+')]);
    }
    assert!(reversed_at(&cells(&mut app, Theme::MONO), "✓ on target"));

    let mut still = onboarded();
    for _ in 0..halves / 2 {
        press(&mut still, &[KeyCode::Char('+')]);
    }
    assert!(
        !reversed_at(&cells(&mut still, Theme::MONO), "✓ on target"),
        "no flash with motion off"
    );
}

#[test]
fn search_picks_out_the_letters_that_matched() {
    let mut app = onboarded();
    press(&mut app, &[KeyCode::Char('a')]);
    type_text(&mut app, "bnch");
    let buffer = cells(&mut app, Theme::MONO);
    let (x, y) = locate(&buffer, "Bench Press");
    let marked: String = (x..x + 11)
        .map(|x| {
            let modifier = buffer[(x, y)].modifier;
            if modifier.contains(Modifier::BOLD | Modifier::UNDERLINED) {
                '^'
            } else {
                ' '
            }
        })
        .collect();
    assert_eq!(marked, "^ ^^^      ");
}

#[test]
fn insights_shows_the_trend_only_when_the_bar_still_fits() {
    let mut app = onboarded();
    app.tab = Tab::Insights;
    assert!(render(&mut app, 80, 24).contains("8 weeks"));
    // The widest target leaves no room for both at the narrowest width.
    app.tab = Tab::Settings;
    press(&mut app, &[KeyCode::Char('l'); 20]);
    assert_eq!(app.target().sets(), crate::domain::Sets::whole(20));
    app.tab = Tab::Insights;
    let narrow = render(&mut app, MIN_WIDTH, 40);
    assert!(!narrow.contains("8 weeks"), "{narrow}");
    assert!(narrow.contains('│'), "the bar keeps its room:\n{narrow}");
}
