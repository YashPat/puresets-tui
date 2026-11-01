//! Rendering. Every function here draws from [`App`] state and the frame
//! size; none of them changes data.

mod home;
mod insights;
mod log_sheet;
mod overlays;
mod settings;
mod theme;
mod why;
mod widgets;

use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    layout::{Constraint, Layout, Position, Rect},
    text::{Line, Span},
    widgets::Paragraph,
};

pub use theme::Theme;

use crate::app::{App, Field, MouseTarget, Overlay, Step, Tab};
use widgets::key_hints;

/// Below this size the layout cannot fit; a message asks for more room.
pub const MIN_WIDTH: u16 = 64;
pub const MIN_HEIGHT: u16 = 20;

/// Where the last frame put the things a mouse can act on.
#[derive(Debug, Default)]
pub struct HitMap {
    regions: Vec<(Rect, MouseTarget)>,
}

impl HitMap {
    /// The target under `position`; later regions sit on top of earlier ones.
    pub fn at(&self, position: Position) -> Option<MouseTarget> {
        self.regions
            .iter()
            .rev()
            .find(|(area, _)| area.contains(position))
            .map(|&(_, target)| target)
    }

    fn add(&mut self, area: Rect, target: MouseTarget) {
        if !area.is_empty() {
            self.regions.push((area, target));
        }
    }
}

/// Draws one frame and returns where its clickable parts ended up.
pub fn draw(frame: &mut Frame<'_>, app: &mut App, theme: Theme) -> HitMap {
    let mut hits = HitMap::default();
    let area = frame.area();
    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        draw_too_small(frame, area, theme);
        return hits;
    }

    let [header, body, footer] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .areas(area);
    let body = body.inner(ratatui::layout::Margin::new(2, 0));

    draw_header(frame, header, app, &mut hits, theme);
    hits.add(body, MouseTarget::Body);
    match app.tab {
        Tab::Home => home::draw(frame, body, app, &mut hits, theme),
        Tab::Insights => insights::draw(frame, body, app, &mut hits, theme),
        Tab::Why => why::draw(frame, body, app, theme),
        Tab::Settings => settings::draw(frame, body, app, &mut hits, theme),
    }
    // Overlays are modal: nothing underneath takes clicks, and a click
    // outside one lands on the backdrop. The footer hints stay live.
    if app.overlay.is_some() {
        hits.regions.clear();
        hits.add(area, MouseTarget::Backdrop);
    }
    draw_footer(frame, footer, app, &mut hits, theme);

    let overlay_area = area.inner(ratatui::layout::Margin::new(1, 1));
    match &mut app.overlay {
        Some(Overlay::Help) => overlays::draw_help(frame, overlay_area, &mut hits, theme),
        Some(Overlay::Onboarding { choice }) => {
            overlays::draw_onboarding(frame, overlay_area, *choice, &mut hits, theme);
        }
        Some(Overlay::Log(sheet)) => {
            log_sheet::draw(
                frame,
                overlay_area,
                sheet,
                &app.tracker,
                app.week,
                &mut hits,
                theme,
            );
        }
        None => {}
    }
    draw_hover(frame, app, &hits, theme);
    hits
}

/// Underlines the text of every region the hovered target owns. Blank cells
/// are skipped so a row's padding stays clean.
fn draw_hover(frame: &mut Frame<'_>, app: &App, hits: &HitMap, theme: Theme) {
    let Some(hovered) = app.hovered else {
        return;
    };
    let buffer = frame.buffer_mut();
    for &(area, target) in &hits.regions {
        if target != hovered {
            continue;
        }
        for position in area.positions() {
            if let Some(cell) = buffer.cell_mut(position)
                && cell.symbol() != " "
            {
                cell.set_style(theme.hover());
            }
        }
    }
}

fn draw_too_small(frame: &mut Frame<'_>, area: Rect, theme: Theme) {
    let lines = vec![
        Line::styled("PureSets needs a bigger window", theme.bold()),
        Line::styled(
            format!(
                "{}×{} now · {MIN_WIDTH}×{MIN_HEIGHT} or larger, please",
                area.width, area.height
            ),
            theme.dim(),
        ),
    ];
    let height = u16::try_from(lines.len()).unwrap_or(2);
    let rect = widgets::centered(area, area.width, height);
    frame.render_widget(Paragraph::new(lines).centered(), rect);
}

fn draw_header(frame: &mut Frame<'_>, area: Rect, app: &App, hits: &mut HitMap, theme: Theme) {
    let mut spans = vec![Span::styled(" PureSets ", theme.accent()), Span::raw("  ")];
    for (index, tab) in Tab::ALL.into_iter().enumerate() {
        let label = format!(" {} {} ", index + 1, tab.title());
        let style = if tab == app.tab {
            theme.tab_active()
        } else {
            theme.dim()
        };
        spans.push(Span::styled(label, style));
        spans.push(Span::raw(" "));
    }
    // Each tab's label is the second-last span pushed for it.
    let areas = widgets::span_areas(area, &spans);
    for (tab, label) in Tab::ALL.into_iter().zip(areas.iter().skip(2).step_by(2)) {
        hits.add(*label, MouseTarget::Tab(tab));
    }
    let tabs = Line::from(spans);
    let target = Line::from(vec![
        Span::styled("target ", theme.dim()),
        Span::styled(app.target().to_string(), theme.bold()),
        Span::styled(" sets/wk ", theme.dim()),
    ])
    .right_aligned();
    // The tabs matter more: the target only shows when both fit.
    let room = usize::from(area.width);
    let fits = tabs.width() + target.width() < room;
    frame.render_widget(tabs, area);
    if fits {
        frame.render_widget(target, area);
    }
}

fn draw_footer(frame: &mut Frame<'_>, area: Rect, app: &App, hits: &mut HitMap, theme: Theme) {
    if let Some(toast) = app.toast_frame() {
        let style = if toast.fading {
            theme.dim()
        } else {
            theme.toast(toast.kind)
        };
        let line = Line::styled(format!(" {}", toast.text), style);
        frame.render_widget(line, area);
        return;
    }
    let mut shown = footer_hints(app);
    let width = |hints: &[(&str, &str)]| key_hints(hints, theme).width() + 1;
    // Drop hints from the middle so the closing help/quit/back hints stay.
    while width(&shown) > usize::from(area.width) && shown.len() > 2 {
        shown.remove(shown.len() - 3);
    }
    let mut line = key_hints(&shown, theme);
    line.spans.insert(0, Span::raw(" "));
    // After the leading space, each hint is a key span and an action span,
    // with a separator span between hints.
    let areas = widgets::span_areas(area, &line.spans);
    for (index, (key, _)) in shown.iter().enumerate() {
        let first = 1 + index * 3;
        let (Some(key_area), Some(action_area), Some(event)) =
            (areas.get(first), areas.get(first + 1), hint_key(key))
        else {
            continue;
        };
        hits.add(key_area.union(*action_area), MouseTarget::Key(event));
    }
    frame.render_widget(line, area);
}

/// The key a footer hint names, if it names exactly one.
fn hint_key(hint: &str) -> Option<KeyEvent> {
    let code = match hint {
        "enter" => KeyCode::Enter,
        "esc" => KeyCode::Esc,
        "tab" => KeyCode::Tab,
        "ctrl-c" => return Some(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
        "ctrl-d" => return Some(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL)),
        _ => {
            let mut chars = hint.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) => KeyCode::Char(c),
                _ => return None,
            }
        }
    };
    Some(KeyEvent::from(code))
}

/// The keys that matter right now, most useful first.
fn footer_hints(app: &App) -> Vec<(&'static str, &'static str)> {
    match &app.overlay {
        Some(Overlay::Help) => vec![("esc", "close help")],
        Some(Overlay::Onboarding { .. }) => {
            vec![("↑↓", "choose"), ("enter", "confirm"), ("ctrl-c", "quit")]
        }
        Some(Overlay::Log(sheet)) => match &sheet.step {
            Step::Pick => vec![
                ("type", "search"),
                ("↑↓", "select"),
                ("enter", "choose"),
                ("tab", "muscle"),
                ("ctrl-d", "delete custom"),
                (
                    "esc",
                    if sheet.query.is_empty() {
                        "close"
                    } else {
                        "clear"
                    },
                ),
            ],
            Step::Sets(_) => vec![
                ("↑↓", "sets"),
                ("0-9", "type"),
                ("enter", "log"),
                ("n", "note"),
                ("esc", "back"),
            ],
            Step::Note(_) => vec![("enter", "save note"), ("esc", "cancel")],
            Step::Create(form) => match form.focus {
                Field::Name => vec![
                    ("type", "name"),
                    ("tab", "muscles"),
                    ("enter", "create"),
                    ("esc", "back"),
                ],
                Field::Muscles => vec![
                    ("arrows", "move"),
                    ("space", "cycle"),
                    ("p/s/x", "primary/secondary/none"),
                    ("enter", "create"),
                    ("esc", "back"),
                ],
            },
            Step::ConfirmDelete { .. } => vec![("y", "delete"), ("n", "keep")],
        },
        None => match app.tab {
            Tab::Home => {
                let mut hints = vec![
                    ("enter", "log"),
                    ("j/k", "muscle"),
                    ("h/l", "week"),
                    ("+/-", "adjust"),
                ];
                if app.can_undo() {
                    hints.push(("u", "undo"));
                }
                hints.extend([("?", "help"), ("q", "quit")]);
                hints
            }
            Tab::Insights => vec![
                ("h/l", "period"),
                ("tab", "next screen"),
                ("?", "help"),
                ("q", "quit"),
            ],
            Tab::Why => vec![
                ("j/k", "scroll"),
                ("tab", "next screen"),
                ("?", "help"),
                ("q", "quit"),
            ],
            Tab::Settings => vec![
                ("h/l", "target"),
                ("m", "moderate"),
                ("o", "optimal"),
                ("?", "help"),
                ("q", "quit"),
            ],
        },
    }
}

#[cfg(test)]
mod mouse_tests;
#[cfg(test)]
mod snapshot_tests;
#[cfg(test)]
mod tests;
