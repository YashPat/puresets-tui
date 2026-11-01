//! Modal overlays: first-run onboarding and the keyboard help.

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Padding, Paragraph, Wrap},
};

use super::{
    HitMap,
    theme::Theme,
    widgets::{centered, row_in, wrapped_row},
};
use crate::{app::MouseTarget, domain::Goal};

fn modal(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &str,
    (width, height): (u16, u16),
    hits: &mut HitMap,
    theme: Theme,
) -> Rect {
    let rect = centered(area, width, height);
    hits.add(rect, MouseTarget::Modal);
    let block = Block::new()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.focus_border())
        .padding(Padding::new(2, 2, 1, 0))
        .title(Span::styled(format!(" {title} "), theme.bold()));
    let inner = block.inner(rect);
    frame.render_widget(Clear, rect);
    frame.render_widget(block, rect);
    inner
}

pub fn draw_onboarding(
    frame: &mut Frame<'_>,
    area: Rect,
    choice: Goal,
    hits: &mut HitMap,
    theme: Theme,
) {
    let inner = modal(frame, area, "Welcome to PureSets", (70, 19), hits, theme);
    let mut lines = vec![
        Line::styled(
            "Track weekly hard sets per muscle. Nothing else.",
            theme.bold(),
        ),
        Line::raw(""),
        Line::styled(
            "Log an exercise and every muscle it works gets credit. Home shows \
             what is on target this week and what still needs work.",
            theme.text(),
        ),
        Line::raw(""),
        Line::styled("Choose a weekly target per muscle:", theme.text()),
        Line::raw(""),
    ];
    for goal in Goal::ALL {
        // A goal's name and blurb lines both pick it.
        let top = wrapped_row(&lines, lines.len(), inner.width);
        for row in [top, top.saturating_add(1)] {
            hits.add(row_in(inner, row), MouseTarget::Goal(goal));
        }
        let active = goal == choice;
        let (marker, style) = if active {
            ("▸ ● ", theme.selected())
        } else {
            ("  ○ ", theme.dim())
        };
        lines.push(Line::from(vec![
            Span::styled(marker, style),
            Span::styled(
                format!("{:<9}", goal.name()),
                if active {
                    theme.selected()
                } else {
                    theme.bold()
                },
            ),
            Span::styled(format!("{} sets", goal.target()), theme.text()),
        ]));
        lines.push(Line::styled(format!("      {}", goal.blurb()), theme.dim()));
    }
    lines.extend([
        Line::raw(""),
        Line::styled(
            "You can change this any time in Settings (4 to 20 sets).",
            theme.dim(),
        ),
    ]);
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

/// A titled group of `(keys, action)` rows.
type HelpSection = (&'static str, &'static [(&'static str, &'static str)]);

const HELP: &[HelpSection] = &[
    (
        "Everywhere",
        &[
            ("1-4 / tab", "switch screen"),
            ("?", "this help"),
            ("q", "quit (ctrl-c works anywhere)"),
            ("mouse", "click or scroll; shift-drag to select text"),
        ],
    ),
    (
        "Home",
        &[
            ("enter", "log sets for the selected muscle"),
            ("a or /", "log sets, search all exercises"),
            ("j/k ↑↓", "select muscle"),
            ("h/l ←→", "previous / next week"),
            ("t", "back to this week"),
            ("+ / -", "add or remove 1 set"),
            ("] / [", "add or remove ½ set"),
            ("u", "undo"),
        ],
    ),
    (
        "Log sheet",
        &[
            ("type", "search exercises"),
            ("↑↓ enter", "choose, then set the count"),
            ("tab", "filter by muscle"),
            ("n", "edit the exercise's note"),
            ("ctrl-d", "delete a custom exercise"),
            ("esc", "back / close"),
        ],
    ),
    (
        "Insights · Why · Settings",
        &[
            ("h/l", "change period / target"),
            ("j/k", "scroll"),
            ("m / o", "moderate / optimal target"),
        ],
    ),
];

/// Wide terminals get two columns of help so it fits without scrolling.
const HELP_TWO_COLUMNS: u16 = 100;

pub fn draw_help(frame: &mut Frame<'_>, area: Rect, hits: &mut HitMap, theme: Theme) {
    let columns: Vec<&[HelpSection]> = if area.width >= HELP_TWO_COLUMNS {
        vec![&HELP[..2], &HELP[2..]]
    } else {
        vec![HELP]
    };
    let column_lines: Vec<Vec<Line<'static>>> = columns
        .iter()
        .map(|sections| help_lines(sections, theme))
        .collect();
    let rows = column_lines.iter().map(Vec::len).max().unwrap_or(0);
    // Borders, top padding and the closing hint.
    let height = u16::try_from(rows + 5).unwrap_or(u16::MAX);
    let width = if columns.len() > 1 { 96 } else { 58 };
    let inner = modal(frame, area, "Keyboard", (width, height), hits, theme);

    let [body, hint] = Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(inner);
    let areas = Layout::horizontal(vec![Constraint::Fill(1); column_lines.len()])
        .spacing(4)
        .split(body);
    for (lines, column) in column_lines.into_iter().zip(areas.iter()) {
        frame.render_widget(Paragraph::new(lines), *column);
    }
    frame.render_widget(Line::styled("esc to close", theme.dim()), hint);
}

fn help_lines(sections: &[HelpSection], theme: Theme) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    for (index, (section, keys)) in sections.iter().enumerate() {
        if index > 0 {
            lines.push(Line::raw(""));
        }
        lines.push(Line::styled((*section).to_owned(), theme.heading()));
        for (key, action) in *keys {
            lines.push(Line::from(vec![
                Span::styled(format!("  {key:<12}"), theme.key()),
                Span::styled((*action).to_owned(), theme.text()),
            ]));
        }
    }
    lines
}
