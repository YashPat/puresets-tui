//! The "Log sets" sheet drawn over the home screen.

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Position, Rect},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, Padding, Paragraph, Wrap},
};

use super::{
    theme::Theme,
    widgets::{Row, centered, fit, status_text},
};
use crate::{
    app::{CreateForm, Field, LogSheet, MAX_SETS, NoteEditor, PickRow, SetCount, Step, TextInput},
    domain::{Category, Exercise, Muscle, Origin, Role, Tracker, Week},
};

const SHEET_WIDTH: u16 = 80;
const SHEET_HEIGHT: u16 = 30;
const LABEL_WIDTH: usize = 8;
const NAME_WIDTH: usize = 30;

pub fn draw(
    frame: &mut Frame<'_>,
    area: Rect,
    sheet: &mut LogSheet,
    tracker: &Tracker,
    week: Week,
    theme: Theme,
) {
    let rect = centered(area, SHEET_WIDTH, SHEET_HEIGHT);
    let title = match &sheet.step {
        Step::Pick => " Log sets ",
        Step::Sets(_) => " How many sets? ",
        Step::Note(_) => " Exercise note ",
        Step::Create(_) => " New custom exercise ",
        Step::ConfirmDelete { .. } => " Delete exercise ",
    };
    let block = Block::new()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.focus_border())
        .padding(Padding::new(2, 2, 1, 0))
        .title(Span::styled(title, theme.bold()))
        .title_bottom(
            Line::styled(
                format!(" Week {} · {} ", week.number(), week.range_label()),
                theme.dim(),
            )
            .right_aligned(),
        );
    let inner = block.inner(rect);
    frame.render_widget(Clear, rect);
    frame.render_widget(block, rect);

    match &sheet.step {
        Step::Pick => draw_pick(frame, inner, sheet, tracker, theme),
        Step::Sets(sets) => draw_sets(frame, inner, sets, tracker, week, theme),
        Step::Note(editor) => draw_note(frame, inner, editor, theme),
        Step::Create(form) => draw_create(frame, inner, form, theme),
        Step::ConfirmDelete { exercise } => draw_confirm(frame, inner, exercise, theme),
    }
}

/// A labelled text field; places the terminal cursor when `focused`.
fn draw_input(
    frame: &mut Frame<'_>,
    area: Rect,
    label: &str,
    input: &TextInput,
    placeholder: &str,
    focused: Focus,
    theme: Theme,
) {
    let label_style = if focused == Focus::On {
        theme.bold()
    } else {
        theme.dim()
    };
    let value = if input.is_empty() {
        Span::styled(placeholder.to_owned(), theme.dim())
    } else {
        Span::styled(input.value().to_owned(), theme.text())
    };
    let line = Line::from(vec![
        Span::styled(format!("{label:<LABEL_WIDTH$}"), label_style),
        value,
    ]);
    frame.render_widget(line, area);
    if focused == Focus::On {
        let column = u16::try_from(LABEL_WIDTH + input.cursor_column()).unwrap_or(u16::MAX);
        let x = area
            .x
            .saturating_add(column)
            .min(area.right().saturating_sub(1));
        frame.set_cursor_position(Position::new(x, area.y));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Focus {
    On,
    Off,
}

fn draw_pick(
    frame: &mut Frame<'_>,
    area: Rect,
    sheet: &mut LogSheet,
    tracker: &Tracker,
    theme: Theme,
) {
    let [search, filter, _, list] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(0),
    ])
    .areas(area);

    draw_input(
        frame,
        search,
        "Search",
        &sheet.query,
        "type an exercise name",
        Focus::On,
        theme,
    );
    let filter_value = sheet.filter.map_or_else(
        || Span::styled("All muscles", theme.text()),
        |muscle| Span::styled(muscle.name(), theme.category(muscle.category())),
    );
    frame.render_widget(
        Line::from(vec![
            Span::styled(format!("{:<LABEL_WIDTH$}", "Muscle"), theme.dim()),
            filter_value,
            Span::styled("   tab / shift-tab to change", theme.dim()),
        ]),
        filter,
    );

    let rows = sheet.rows(tracker);
    let selected = sheet.selected.min(rows.len().saturating_sub(1));
    let mut items = Vec::new();
    let mut selected_item = 0;
    let mut section = None;
    let has_exercises = rows
        .iter()
        .any(|row| matches!(row, PickRow::Exercise { .. }));
    for (index, row) in rows.iter().enumerate() {
        let is_selected = index == selected;
        match row {
            PickRow::Exercise {
                exercise,
                section: row_section,
            } => {
                if section != Some(*row_section) {
                    if section.is_some() {
                        items.push(ListItem::new(""));
                    }
                    items.push(ListItem::new(Line::styled(
                        row_section.title(),
                        theme.dim(),
                    )));
                    section = Some(*row_section);
                }
                if is_selected {
                    selected_item = items.len();
                }
                let row = if is_selected {
                    Row::Selected
                } else {
                    Row::Plain
                };
                items.push(ListItem::new(exercise_line(exercise, row, theme)));
            }
            PickRow::NewCustom => {
                items.push(ListItem::new(""));
                if !has_exercises {
                    let message = match sheet.filter {
                        Some(muscle) if sheet.query.is_empty() => {
                            format!("No exercises for {muscle}.")
                        }
                        _ => format!("No matches for “{}”.", sheet.query.value()),
                    };
                    items.push(ListItem::new(Line::styled(message, theme.dim())));
                }
                if is_selected {
                    selected_item = items.len();
                }
                let label = if sheet.query.is_empty() {
                    "+ Create a custom exercise".to_owned()
                } else {
                    format!("+ Create “{}”", sheet.query.value().trim())
                };
                let style = if is_selected {
                    theme.selected()
                } else {
                    theme.accent()
                };
                let marker = if is_selected { "▸ " } else { "  " };
                items.push(ListItem::new(Line::from(vec![
                    Span::styled(marker, theme.selected()),
                    Span::styled(label, style),
                ])));
            }
        }
    }
    // Show the section header with the first row instead of scrolling past it.
    if selected_item == 1 {
        *sheet.list.offset_mut() = 0;
    }
    sheet.list.select(Some(selected_item));
    frame.render_stateful_widget(List::new(items), list, &mut sheet.list);
}

fn exercise_line(exercise: &Exercise, row: Row, theme: Theme) -> Line<'static> {
    let marker = row.marker();
    let name_style = row.style(theme);
    let mut spans = vec![
        Span::styled(marker, theme.selected()),
        Span::styled(fit(exercise.name(), NAME_WIDTH), name_style),
        Span::raw(" "),
    ];
    let mut tags = Vec::new();
    if exercise.origin() == Origin::Custom {
        tags.push("custom");
    }
    if !exercise.note().is_empty() {
        tags.push("✎");
    }
    spans.push(Span::styled(fit(&tags.join(" "), 9), theme.accent()));
    spans.push(Span::styled(muscle_summary(exercise), theme.dim()));
    Line::from(spans)
}

/// "Chest, Triceps + Front Delts": primary muscles, then secondary.
fn muscle_summary(exercise: &Exercise) -> String {
    let names = |muscles: &[Muscle]| {
        muscles
            .iter()
            .map(|muscle| muscle.name())
            .collect::<Vec<_>>()
            .join(", ")
    };
    if exercise.secondary().is_empty() {
        names(exercise.primary())
    } else {
        format!(
            "{} + {}",
            names(exercise.primary()),
            names(exercise.secondary())
        )
    }
}

fn draw_sets(
    frame: &mut Frame<'_>,
    area: Rect,
    sets: &SetCount,
    tracker: &Tracker,
    week: Week,
    theme: Theme,
) {
    let Some(exercise) = tracker.catalog.get(&sets.exercise) else {
        frame.render_widget(
            Line::styled("This exercise no longer exists.", theme.error()),
            area,
        );
        return;
    };
    let target = tracker.target_or_default();
    let mut lines = vec![Line::styled(exercise.name().to_owned(), theme.bold())];
    lines.push(if exercise.note().is_empty() {
        Line::styled("n to add a note", theme.dim())
    } else {
        Line::from(vec![
            Span::styled("✎ ", theme.accent()),
            Span::styled(exercise.note().to_owned(), theme.text()),
        ])
    });
    lines.push(Line::raw(""));
    let unit = if sets.count == 1 {
        "hard set"
    } else {
        "hard sets"
    };
    lines.push(Line::from(vec![
        Span::styled(
            "‹ ",
            if sets.count > 1 {
                theme.key()
            } else {
                theme.dim()
            },
        ),
        Span::styled(format!("{:>2}", sets.count), theme.accent()),
        Span::styled(
            " ›",
            if sets.count < MAX_SETS {
                theme.key()
            } else {
                theme.dim()
            },
        ),
        Span::styled(format!("  {unit}"), theme.text()),
        Span::styled("     ↑↓ or type a number", theme.dim()),
    ]));
    lines.push(Line::raw(""));
    lines.push(Line::styled(
        format!(
            "{}{}{}{}",
            fit("MUSCLE", 16),
            fit("ROLE", 11),
            fit("CREDIT", 8),
            "THIS WEEK"
        ),
        theme.dim(),
    ));
    for credit in exercise.credits(sets.count) {
        let before = tracker.log.sets(week, credit.muscle);
        let after = before + credit.sets;
        let role_style = match credit.role {
            Role::Primary => theme.text(),
            Role::Secondary => theme.dim(),
        };
        let status_style = if after >= target.sets() {
            theme.success()
        } else {
            theme.dim()
        };
        lines.push(Line::from(vec![
            Span::styled(
                fit(credit.muscle.name(), 16),
                theme.category(credit.muscle.category()),
            ),
            Span::styled(fit(credit.role.label(), 11), role_style),
            Span::styled(fit(&format!("+{}", credit.sets), 8), theme.bold()),
            Span::styled(format!("{before} → "), theme.dim()),
            Span::styled(after.to_string(), theme.bold()),
            Span::styled(format!(" / {target}   "), theme.dim()),
            Span::styled(status_text(after, target), status_style),
        ]));
    }
    lines.push(Line::raw(""));
    lines.push(Line::styled(
        "Primary muscles get 1 set per set, secondary muscles ½.",
        theme.dim(),
    ));
    frame.render_widget(Paragraph::new(lines), area);
}

fn draw_note(frame: &mut Frame<'_>, area: Rect, editor: &NoteEditor, theme: Theme) {
    let [name, _, input, _, help] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(0),
    ])
    .areas(area);
    frame.render_widget(Line::styled(editor.exercise.clone(), theme.bold()), name);
    draw_input(
        frame,
        input,
        "Note",
        &editor.input,
        "e.g. seat height 4, use the rope",
        Focus::On,
        theme,
    );
    frame.render_widget(
        Paragraph::new(vec![
            Line::styled("Shown each time you log this exercise.", theme.dim()),
            Line::styled("Leave it empty to remove the note.", theme.dim()),
        ])
        .wrap(Wrap { trim: false }),
        help,
    );
}

fn draw_create(frame: &mut Frame<'_>, area: Rect, form: &CreateForm, theme: Theme) {
    let [name, _, heading, _, grid, _, legend, error] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(7),
        Constraint::Length(1),
        Constraint::Length(2),
        Constraint::Min(0),
    ])
    .areas(area);

    let name_focus = if form.focus == Field::Name {
        Focus::On
    } else {
        Focus::Off
    };
    draw_input(
        frame,
        name,
        "Name",
        &form.name,
        "e.g. Cable Fly",
        name_focus,
        theme,
    );
    let heading_style = if name_focus == Focus::Off {
        theme.bold()
    } else {
        theme.dim()
    };
    frame.render_widget(
        Line::from(vec![
            Span::styled("Muscles", heading_style),
            Span::styled("   which muscles does it work?", theme.dim()),
        ]),
        heading,
    );

    let columns = Layout::horizontal([Constraint::Fill(1); 4]).split(grid);
    for (column, category) in columns.iter().zip(Category::ALL) {
        let mut lines = vec![Line::styled(
            category.name().to_uppercase(),
            theme.category(category),
        )];
        for muscle in category.muscles() {
            let at_cursor = name_focus == Focus::Off && form.cursor == muscle;
            let (symbol, symbol_style) = match form.role(muscle) {
                Some(Role::Primary) => ("● ", theme.success()),
                Some(Role::Secondary) => ("◐ ", theme.accent()),
                None => ("○ ", theme.dim()),
            };
            let name_style = if at_cursor {
                theme.selected()
            } else if form.role(muscle).is_some() {
                theme.text()
            } else {
                theme.dim()
            };
            lines.push(Line::from(vec![
                Span::styled(if at_cursor { "▸" } else { " " }, theme.selected()),
                Span::styled(symbol, symbol_style),
                Span::styled(muscle.name(), name_style),
            ]));
        }
        frame.render_widget(Paragraph::new(lines), *column);
    }

    frame.render_widget(
        Paragraph::new(vec![Line::from(vec![
            Span::styled("● ", theme.success()),
            Span::styled("primary, 1 set per set   ", theme.dim()),
            Span::styled("◐ ", theme.accent()),
            Span::styled("secondary, ½ set per set", theme.dim()),
        ])]),
        legend,
    );
    if let Some(message) = &form.error {
        frame.render_widget(Line::styled(format!("✗ {message}"), theme.error()), error);
    }
}

fn draw_confirm(frame: &mut Frame<'_>, area: Rect, exercise: &str, theme: Theme) {
    let lines = vec![
        Line::from(vec![
            Span::styled("Delete ", theme.text()),
            Span::styled(format!("“{exercise}”"), theme.bold()),
            Span::styled("?", theme.text()),
        ]),
        Line::raw(""),
        Line::styled(
            "It leaves your exercise list. Sets you already logged stay in your history.",
            theme.dim(),
        ),
        Line::raw(""),
        Line::from(vec![
            Span::styled("y ", theme.key()),
            Span::styled("delete    ", theme.error()),
            Span::styled("n ", theme.key()),
            Span::styled("keep it", theme.text()),
        ]),
    ];
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
}
