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
    layout::{Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::Paragraph,
};

pub use theme::Theme;

use crate::app::{App, Field, Overlay, Step, Tab};
use widgets::key_hints;

/// Below this size the layout cannot fit; a message asks for more room.
pub const MIN_WIDTH: u16 = 64;
pub const MIN_HEIGHT: u16 = 20;

pub fn draw(frame: &mut Frame<'_>, app: &mut App, theme: Theme) {
    let area = frame.area();
    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        draw_too_small(frame, area, theme);
        return;
    }

    let [header, body, footer] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .areas(area);
    let body = body.inner(ratatui::layout::Margin::new(2, 0));

    draw_header(frame, header, app, theme);
    match app.tab {
        Tab::Home => home::draw(frame, body, app, theme),
        Tab::Insights => insights::draw(frame, body, app, theme),
        Tab::Why => why::draw(frame, body, app, theme),
        Tab::Settings => settings::draw(frame, body, app, theme),
    }
    draw_footer(frame, footer, app, theme);

    let overlay_area = area.inner(ratatui::layout::Margin::new(1, 1));
    match &mut app.overlay {
        Some(Overlay::Help) => overlays::draw_help(frame, overlay_area, theme),
        Some(Overlay::Onboarding { choice }) => {
            overlays::draw_onboarding(frame, overlay_area, *choice, theme);
        }
        Some(Overlay::Log(sheet)) => {
            log_sheet::draw(frame, overlay_area, sheet, &app.tracker, app.week, theme);
        }
        None => {}
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

fn draw_header(frame: &mut Frame<'_>, area: Rect, app: &App, theme: Theme) {
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

fn draw_footer(frame: &mut Frame<'_>, area: Rect, app: &App, theme: Theme) {
    let line = if let Some(toast) = &app.toast {
        Line::styled(format!(" {}", toast.message), theme.toast(toast.kind))
    } else {
        let mut hints = footer_hints(app);
        let width = |hints: &[(&str, &str)]| key_hints(hints, theme).width() + 1;
        // Drop hints from the middle so the closing help/quit/back hints stay.
        while width(&hints) > usize::from(area.width) && hints.len() > 2 {
            hints.remove(hints.len() - 3);
        }
        let mut line = key_hints(&hints, theme);
        line.spans.insert(0, Span::raw(" "));
        line
    };
    frame.render_widget(line, area);
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
mod tests;
