//! Mouse tests: draw a frame, find text on screen, and click or scroll where
//! it landed, as a terminal would report it.

use std::time::Instant;

use jiff::civil::date;
use ratatui::{
    Terminal,
    backend::TestBackend,
    buffer::Buffer,
    crossterm::event::{KeyCode, KeyEvent},
    layout::Position,
};

use super::{HitMap, Theme, draw};
use crate::{
    app::{App, MouseAction, MouseTarget, Overlay, Tab},
    domain::{Catalog, Muscle, Tracker},
};

fn onboarded() -> App {
    let tracker = Tracker::new(Catalog::built_in().expect("bundled catalog parses"));
    let mut app = App::new(
        tracker,
        date(2026, 11, 1),
        Instant::now(),
        "~/data.json".into(),
    );
    app.handle_key(KeyEvent::from(KeyCode::Enter));
    app
}

/// Draws one frame and returns the screen and where its targets are.
fn frame(app: &mut App, width: u16, height: u16) -> (Buffer, HitMap) {
    let mut terminal =
        Terminal::new(TestBackend::new(width, height)).expect("test backend never fails");
    let mut hits = HitMap::default();
    terminal
        .draw(|frame| hits = draw(frame, app, Theme::COLOR))
        .expect("drawing to a test backend never fails");
    (terminal.backend().buffer().clone(), hits)
}

/// The first cell, from row `from_y` down, where `text` starts.
fn find(buffer: &Buffer, text: &str, from_y: u16) -> Option<Position> {
    let chars: Vec<String> = text.chars().map(String::from).collect();
    let width = buffer.area.width;
    let len = u16::try_from(chars.len()).ok()?;
    (from_y..buffer.area.height).find_map(|y| {
        (0..width.saturating_sub(len)).find_map(|x| {
            chars
                .iter()
                .zip(x..)
                .all(|(c, x)| buffer[(x, y)].symbol() == c)
                .then_some(Position::new(x, y))
        })
    })
}

/// Clicks or scrolls at `position` the way `main` does.
fn mouse(app: &mut App, hits: &HitMap, position: Position, action: MouseAction) {
    if let Some(target) = hits.at(position) {
        app.handle_mouse(target, action);
    }
}

#[test]
fn clicking_a_tab_label_switches_to_it() {
    let mut app = onboarded();
    for tab in Tab::ALL.into_iter().rev() {
        let (buffer, hits) = frame(&mut app, 80, 24);
        let label = find(&buffer, tab.title(), 0).expect("every tab title is in the header");
        assert_eq!(label.y, 0, "{} is in the header row", tab.title());
        assert_eq!(hits.at(label), Some(MouseTarget::Tab(tab)));
        mouse(&mut app, &hits, label, MouseAction::Click);
        assert_eq!(app.tab, tab);
    }
}

#[test]
fn clicking_a_muscle_row_selects_it() {
    for (width, height) in [(80, 24), (120, 40)] {
        let mut app = onboarded();
        let mut clicked = 0;
        for muscle in Muscle::ALL {
            let (buffer, hits) = frame(&mut app, width, height);
            // Below the header and week bar; skip the far end of the row.
            let Some(name) = find(&buffer, muscle.name(), 4) else {
                continue;
            };
            mouse(&mut app, &hits, name, MouseAction::Click);
            assert_eq!(app.selected, muscle, "at {width}x{height}");
            clicked += 1;
        }
        // Both columns fit at 120 wide; at 80 the list scrolls.
        let expected = if width >= 110 { Muscle::ALL.len() } else { 10 };
        assert!(
            clicked >= expected,
            "only {clicked} muscles were clickable at {width}x{height}"
        );
    }
}

#[test]
fn clicking_outside_the_rows_changes_nothing() {
    let mut app = onboarded();
    let selected = app.selected;
    let (_, hits) = frame(&mut app, 80, 24);
    // The week bar and the blank line under it.
    for y in [2, 3] {
        mouse(&mut app, &hits, Position::new(10, y), MouseAction::Click);
    }
    assert_eq!(app.selected, selected);
    assert_eq!(app.tab, Tab::Home);
}

#[test]
fn wheel_scrolls_the_why_text() {
    let mut app = onboarded();
    app.tab = Tab::Why;
    let (_, hits) = frame(&mut app, 80, 24);
    let body = Position::new(10, 12);
    assert_eq!(hits.at(body), Some(MouseTarget::WhyText));

    mouse(&mut app, &hits, body, MouseAction::ScrollDown);
    mouse(&mut app, &hits, body, MouseAction::ScrollDown);
    assert_eq!(app.why_scroll, 6);
    mouse(&mut app, &hits, body, MouseAction::ScrollUp);
    assert_eq!(app.why_scroll, 3);
    // Clicking the text does not scroll it.
    mouse(&mut app, &hits, body, MouseAction::Click);
    assert_eq!(app.why_scroll, 3);
}

/// The log sheet's selected row, if it is open.
fn log_selection(app: &App) -> Option<usize> {
    match &app.overlay {
        Some(Overlay::Log(sheet)) => Some(sheet.selected),
        _ => None,
    }
}

#[test]
fn wheel_moves_the_log_sheet_selection_like_the_arrow_keys() {
    let mut by_wheel = onboarded();
    let mut by_keys = onboarded();
    for app in [&mut by_wheel, &mut by_keys] {
        app.handle_key(KeyEvent::from(KeyCode::Char('a')));
    }

    let (buffer, hits) = frame(&mut by_wheel, 80, 24);
    let row = find(&buffer, "▸", 3).expect("the sheet shows a selected row");
    assert_eq!(hits.at(row), Some(MouseTarget::LogList));
    for _ in 0..3 {
        mouse(&mut by_wheel, &hits, row, MouseAction::ScrollDown);
        by_keys.handle_key(KeyEvent::from(KeyCode::Down));
    }
    mouse(&mut by_wheel, &hits, row, MouseAction::ScrollUp);
    by_keys.handle_key(KeyEvent::from(KeyCode::Up));
    assert_eq!(log_selection(&by_wheel), Some(2));
    assert_eq!(log_selection(&by_wheel), log_selection(&by_keys));
}

#[test]
fn wheel_does_nothing_on_later_log_sheet_steps() {
    let mut app = onboarded();
    app.handle_key(KeyEvent::from(KeyCode::Char('a')));
    let (buffer, hits) = frame(&mut app, 80, 24);
    let row = find(&buffer, "▸", 3).expect("the sheet shows a selected row");
    // Choose the exercise: the sets step uses up/down for the count.
    app.handle_key(KeyEvent::from(KeyCode::Enter));
    let before = app.overlay.clone();
    mouse(&mut app, &hits, row, MouseAction::ScrollDown);
    assert_eq!(app.overlay, before);
}

#[test]
fn overlays_block_clicks_underneath() {
    let mut app = onboarded();
    app.handle_key(KeyEvent::from(KeyCode::Char('?')));
    let (buffer, hits) = frame(&mut app, 120, 40);
    let tab = find(&buffer, "Settings", 0).expect("the header shows under the help");
    assert_eq!(hits.at(tab), None);
    // Even a target left over from an older frame is ignored.
    app.handle_mouse(MouseTarget::Tab(Tab::Settings), MouseAction::Click);
    assert_eq!(app.tab, Tab::Home);
    assert_eq!(app.overlay, Some(Overlay::Help));
}

#[test]
fn too_small_screens_have_no_targets() {
    let mut app = onboarded();
    let (_, hits) = frame(&mut app, 40, 10);
    assert_eq!(hits.at(Position::new(5, 0)), None);
}
