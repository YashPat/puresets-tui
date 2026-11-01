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
    app::{App, Field, MouseAction, MouseTarget, Overlay, Step, Tab},
    domain::{Catalog, Goal, Muscle, Period, Role, Tracker},
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
            let was_selected = app.selected == muscle;
            mouse(&mut app, &hits, name, MouseAction::Click);
            assert_eq!(app.selected, muscle, "at {width}x{height}");
            if was_selected {
                // A second click opens the log sheet; close it again.
                assert!(matches!(app.overlay, Some(Overlay::Log(_))));
                app.handle_key(KeyEvent::from(KeyCode::Esc));
            }
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
    assert_eq!(hits.at(body), Some(MouseTarget::Body));

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
    assert_eq!(hits.at(row), Some(MouseTarget::LogRow(0)));
    for _ in 0..3 {
        mouse(&mut by_wheel, &hits, row, MouseAction::ScrollDown);
        by_keys.handle_key(KeyEvent::from(KeyCode::Down));
    }
    mouse(&mut by_wheel, &hits, row, MouseAction::ScrollUp);
    by_keys.handle_key(KeyEvent::from(KeyCode::Up));
    assert_eq!(log_selection(&by_wheel), Some(2));
    assert_eq!(log_selection(&by_wheel), log_selection(&by_keys));
}

/// The log sheet's step, if it is open.
fn log_step(app: &App) -> Option<&Step> {
    match &app.overlay {
        Some(Overlay::Log(sheet)) => Some(&sheet.step),
        _ => None,
    }
}

/// The set count on the log sheet's sets step.
fn set_count(app: &App) -> Option<u32> {
    match log_step(app) {
        Some(Step::Sets(sets)) => Some(sets.count),
        _ => None,
    }
}

#[test]
fn clicking_a_log_row_selects_it_and_a_second_click_chooses_it() {
    let mut app = onboarded();
    app.handle_key(KeyEvent::from(KeyCode::Char('a')));
    let (buffer, hits) = frame(&mut app, 80, 24);
    let first = find(&buffer, "▸", 3).expect("the sheet shows a selected row");
    let third = Position::new(first.x + 4, first.y + 2);
    assert_eq!(hits.at(third), Some(MouseTarget::LogRow(2)));

    mouse(&mut app, &hits, third, MouseAction::Click);
    assert_eq!(log_selection(&app), Some(2));
    assert_eq!(log_step(&app), Some(&Step::Pick));
    let (_, hits) = frame(&mut app, 80, 24);
    mouse(&mut app, &hits, third, MouseAction::Click);
    assert_eq!(set_count(&app), Some(1));
}

#[test]
fn the_set_count_arrows_and_wheel_change_the_count() {
    let mut app = onboarded();
    app.handle_key(KeyEvent::from(KeyCode::Char('a')));
    app.handle_key(KeyEvent::from(KeyCode::Enter));
    let (buffer, hits) = frame(&mut app, 80, 24);
    let up = find(&buffer, "›", 3).expect("the count has a raise arrow");
    let down = find(&buffer, "‹", 3).expect("the count has a lower arrow");
    mouse(&mut app, &hits, up, MouseAction::Click);
    mouse(&mut app, &hits, up, MouseAction::Click);
    assert_eq!(set_count(&app), Some(3));
    mouse(&mut app, &hits, down, MouseAction::Click);
    assert_eq!(set_count(&app), Some(2));
    mouse(&mut app, &hits, up, MouseAction::ScrollUp);
    assert_eq!(set_count(&app), Some(3));
}

#[test]
fn clicking_outside_the_log_sheet_closes_it_only_while_picking() {
    let mut app = onboarded();
    app.handle_key(KeyEvent::from(KeyCode::Char('a')));
    let corner = Position::new(0, 1);
    let (_, hits) = frame(&mut app, 120, 40);
    assert_eq!(hits.at(corner), Some(MouseTarget::Backdrop));

    // Mid-log, a stray click must not throw the count away.
    app.handle_key(KeyEvent::from(KeyCode::Enter));
    let (_, hits) = frame(&mut app, 120, 40);
    mouse(&mut app, &hits, corner, MouseAction::Click);
    assert_eq!(set_count(&app), Some(1));

    app.handle_key(KeyEvent::from(KeyCode::Esc));
    let (_, hits) = frame(&mut app, 120, 40);
    mouse(&mut app, &hits, corner, MouseAction::Click);
    assert_eq!(app.overlay, None);
}

#[test]
fn clicking_the_create_grid_cycles_a_muscles_role() {
    let mut app = onboarded();
    app.handle_key(KeyEvent::from(KeyCode::Char('a')));
    for c in "zzqx".chars() {
        app.handle_key(KeyEvent::from(KeyCode::Char(c)));
    }
    // Nothing matches, so the only row creates a custom exercise.
    app.handle_key(KeyEvent::from(KeyCode::Enter));
    let (buffer, hits) = frame(&mut app, 120, 40);
    let name = find(&buffer, &format!("○ {}", Muscle::Biceps.name()), 3)
        .expect("the grid lists every muscle");
    mouse(&mut app, &hits, name, MouseAction::Click);
    let Some(Step::Create(form)) = log_step(&app) else {
        panic!("still on the create step: {:?}", log_step(&app));
    };
    assert_eq!(form.focus, Field::Muscles);
    assert_eq!(form.cursor, Muscle::Biceps);
    assert_eq!(form.role(Muscle::Biceps), Some(Role::Primary));
}

#[test]
fn wheel_on_home_moves_the_selected_muscle() {
    let mut by_wheel = onboarded();
    let mut by_keys = onboarded();
    let (_, hits) = frame(&mut by_wheel, 80, 24);
    let body = Position::new(10, 8);
    mouse(&mut by_wheel, &hits, body, MouseAction::ScrollDown);
    mouse(&mut by_wheel, &hits, body, MouseAction::ScrollDown);
    by_keys.handle_key(KeyEvent::from(KeyCode::Down));
    by_keys.handle_key(KeyEvent::from(KeyCode::Down));
    assert_ne!(by_wheel.selected, Muscle::ALL[0]);
    assert_eq!(by_wheel.selected, by_keys.selected);
}

#[test]
fn the_week_bar_arrows_change_the_week() {
    let mut app = onboarded();
    let current = app.current_week();
    app.tracker.first_week = Some(current.previous().previous());

    let (buffer, hits) = frame(&mut app, 80, 24);
    let back = find(&buffer, "‹", 2).expect("the week bar has a back arrow");
    mouse(&mut app, &hits, back, MouseAction::Click);
    mouse(&mut app, &hits, back, MouseAction::Click);
    assert_eq!(app.week, current.previous().previous());

    let (buffer, hits) = frame(&mut app, 80, 24);
    let forward = find(&buffer, "›", 2).expect("the week bar has a forward arrow");
    mouse(&mut app, &hits, forward, MouseAction::Click);
    assert_eq!(app.week, current.previous());

    let (buffer, hits) = frame(&mut app, 80, 24);
    let today = find(&buffer, "back to", 2).expect("an old week offers a way back");
    mouse(&mut app, &hits, today, MouseAction::Click);
    assert_eq!(app.week, current);
}

#[test]
fn clicking_a_period_shows_it() {
    let mut app = onboarded();
    app.tab = Tab::Insights;
    for period in [Period::Year, Period::FourWeeks, Period::ThisWeek] {
        let (buffer, hits) = frame(&mut app, 80, 24);
        let label = find(&buffer, period.label(), 2).expect("every period is listed");
        mouse(&mut app, &hits, label, MouseAction::Click);
        assert_eq!(app.period, period);
    }
}

#[test]
fn the_settings_arrows_and_presets_change_the_target() {
    let mut app = onboarded();
    app.tab = Tab::Settings;
    let start = app.target().get();

    let (buffer, hits) = frame(&mut app, 80, 24);
    let up = find(&buffer, "›", 2).expect("the target has a raise arrow");
    mouse(&mut app, &hits, up, MouseAction::Click);
    assert_eq!(app.target().get(), start + 1);

    let (buffer, hits) = frame(&mut app, 80, 24);
    let down = find(&buffer, "‹", 2).expect("the target has a lower arrow");
    mouse(&mut app, &hits, down, MouseAction::Click);
    assert_eq!(app.target().get(), start);

    for goal in [Goal::Optimal, Goal::Moderate] {
        let (buffer, hits) = frame(&mut app, 80, 24);
        // Below the current preset's name on the target line.
        let row = find(&buffer, goal.name(), 6).expect("every preset is listed");
        mouse(&mut app, &hits, row, MouseAction::Click);
        assert_eq!(app.target(), goal.target());
    }
}

#[test]
fn onboarding_goals_are_chosen_by_clicking() {
    let tracker = Tracker::new(Catalog::built_in().expect("bundled catalog parses"));
    let mut app = App::new(
        tracker,
        date(2026, 11, 1),
        Instant::now(),
        "~/data.json".into(),
    );
    let (buffer, hits) = frame(&mut app, 80, 24);
    let optimal = find(&buffer, Goal::Optimal.name(), 2).expect("onboarding lists the goals");
    mouse(&mut app, &hits, optimal, MouseAction::Click);
    assert_eq!(
        app.overlay,
        Some(Overlay::Onboarding {
            choice: Goal::Optimal
        })
    );
    // Clicking the chosen goal again confirms it.
    let (_, hits) = frame(&mut app, 80, 24);
    mouse(&mut app, &hits, optimal, MouseAction::Click);
    assert_eq!(app.overlay, None);
    assert_eq!(app.target(), Goal::Optimal.target());
}

#[test]
fn footer_hints_press_their_key() {
    let mut app = onboarded();
    // The welcome toast covers the hints until it times out.
    app.toast = None;
    let (buffer, hits) = frame(&mut app, 120, 40);
    let help = find(&buffer, "help", 39).expect("the footer offers help");
    mouse(&mut app, &hits, help, MouseAction::Click);
    assert_eq!(app.overlay, Some(Overlay::Help));

    let (buffer, hits) = frame(&mut app, 120, 40);
    let close = find(&buffer, "close help", 39).expect("the footer offers a way out");
    mouse(&mut app, &hits, close, MouseAction::Click);
    assert_eq!(app.overlay, None);
}

#[test]
fn clicking_outside_help_closes_it() {
    let mut app = onboarded();
    app.handle_key(KeyEvent::from(KeyCode::Char('?')));
    let (buffer, hits) = frame(&mut app, 120, 40);
    let inside = find(&buffer, "switch screen", 0).expect("help lists its keys");
    mouse(&mut app, &hits, inside, MouseAction::Click);
    assert_eq!(app.overlay, Some(Overlay::Help));
    mouse(&mut app, &hits, Position::new(0, 1), MouseAction::Click);
    assert_eq!(app.overlay, None);
}

#[test]
fn overlays_block_clicks_underneath() {
    let mut app = onboarded();
    app.handle_key(KeyEvent::from(KeyCode::Char('?')));
    let (buffer, hits) = frame(&mut app, 120, 40);
    let tab = find(&buffer, "Settings", 0).expect("the header shows under the help");
    assert_eq!(hits.at(tab), Some(MouseTarget::Backdrop));
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
