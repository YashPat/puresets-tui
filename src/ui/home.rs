//! Home: this week's sets for every muscle, grouped by category.

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::Paragraph,
};

use super::{
    HitMap,
    theme::Theme,
    widgets::{Row, fit, set_bar, set_bar_width, status_text},
};
use crate::{
    app::{App, MouseTarget},
    domain::{Category, Muscle, Sets, Target},
};

/// Width at which the categories are shown in two columns.
const TWO_COLUMNS: u16 = 110;
const NAME_WIDTH: usize = 14;
/// "▸ " marker before the name.
const MARKER_WIDTH: u16 = 2;
/// " 12.5 / 12 " count column.
const COUNT_WIDTH: u16 = 12;
const STATUS_WIDTH: u16 = 12;

pub fn draw(frame: &mut Frame<'_>, area: Rect, app: &mut App, hits: &mut HitMap, theme: Theme) {
    let [week_bar, _, content] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(0),
    ])
    .areas(area);
    draw_week_bar(frame, week_bar, app, theme);

    if area.width >= TWO_COLUMNS {
        let [left, _, right] = Layout::horizontal([
            Constraint::Fill(1),
            Constraint::Length(4),
            Constraint::Fill(1),
        ])
        .areas(content);
        let left_lines = category_lines(app, &[Category::Push, Category::Pull], left.width, theme);
        let right_lines =
            category_lines(app, &[Category::Legs, Category::Core], right.width, theme);
        let selected_line = [&left_lines, &right_lines]
            .into_iter()
            .find_map(|lines| lines.selected)
            .unwrap_or(0);
        let height = left_lines.lines.len().max(right_lines.lines.len());
        let scroll = scroll_to(app, selected_line, height, content.height);
        add_muscle_hits(hits, left, &left_lines.muscles, scroll);
        add_muscle_hits(hits, right, &right_lines.muscles, scroll);
        frame.render_widget(Paragraph::new(left_lines.lines).scroll((scroll, 0)), left);
        frame.render_widget(Paragraph::new(right_lines.lines).scroll((scroll, 0)), right);
    } else {
        let lines = category_lines(app, &Category::ALL, content.width, theme);
        let height = lines.lines.len();
        let scroll = scroll_to(app, lines.selected.unwrap_or(0), height, content.height);
        add_muscle_hits(hits, content, &lines.muscles, scroll);
        frame.render_widget(Paragraph::new(lines.lines).scroll((scroll, 0)), content);
    }
}

/// Makes each visible muscle row clickable across the column's width.
fn add_muscle_hits(hits: &mut HitMap, area: Rect, muscles: &[(usize, Muscle)], scroll: u16) {
    for &(line, muscle) in muscles {
        let Some(row) = line
            .checked_sub(usize::from(scroll))
            .and_then(|row| u16::try_from(row).ok())
            .filter(|&row| row < area.height)
        else {
            continue;
        };
        let row_area = Rect {
            y: area.y + row,
            height: 1,
            ..area
        };
        hits.add(row_area, MouseTarget::Muscle(muscle));
    }
}

/// Keeps the selected line visible, remembering the offset between frames.
fn scroll_to(app: &mut App, selected: usize, total: usize, height: u16) -> u16 {
    let height = usize::from(height).max(1);
    let max_scroll = total.saturating_sub(height);
    let mut scroll = app.home_scroll.min(max_scroll);
    // The category header sits one line above its first muscle; keep it in view too.
    let top = selected.saturating_sub(1);
    if top < scroll {
        scroll = top;
    } else if selected >= scroll + height {
        scroll = selected + 1 - height;
    }
    app.home_scroll = scroll;
    u16::try_from(scroll).unwrap_or(u16::MAX)
}

fn draw_week_bar(frame: &mut Frame<'_>, area: Rect, app: &App, theme: Theme) {
    let week = app.week;
    let current = app.current_week();
    let can_go_back = week > app.earliest_week();
    let can_go_forward = week < current;
    let arrow = |shown: bool, glyph: &'static str| {
        if shown {
            Span::styled(glyph, theme.key())
        } else {
            Span::styled(glyph, theme.dim())
        }
    };

    let mut spans = vec![
        arrow(can_go_back, "‹ "),
        Span::styled(format!("Week {}", week.number()), theme.bold()),
        Span::styled(format!("  {}", week.range_label()), theme.text()),
        arrow(can_go_forward, " ›"),
        Span::raw("   "),
    ];
    if week == current {
        spans.push(Span::styled("this week", theme.success()));
    } else {
        spans.push(Span::styled("t ", theme.key()));
        spans.push(Span::styled("back to this week", theme.dim()));
    }

    let total = app.tracker.log.week_total(week);
    let target = app.target();
    let on_target = Muscle::ALL
        .into_iter()
        .filter(|&muscle| app.tracker.log.sets(week, muscle) >= target.sets())
        .count();
    let summary = Line::from(vec![
        Span::styled(total.to_string(), theme.bold()),
        Span::styled(" sets  ·  ", theme.dim()),
        Span::styled(format!("{on_target}/{}", Muscle::ALL.len()), theme.bold()),
        Span::styled(" on target", theme.dim()),
    ])
    .right_aligned();

    let [left, right] =
        Layout::horizontal([Constraint::Min(0), Constraint::Length(30)]).areas(area);
    frame.render_widget(Line::from(spans), left);
    frame.render_widget(summary, right);
}

struct CategoryLines {
    lines: Vec<Line<'static>>,
    /// Index of the selected muscle's line, if it is in these categories.
    selected: Option<usize>,
    /// The line each muscle sits on.
    muscles: Vec<(usize, Muscle)>,
}

fn category_lines(app: &App, categories: &[Category], width: u16, theme: Theme) -> CategoryLines {
    let target = app.target();
    let mut lines = Vec::new();
    let mut selected = None;
    let mut rows = Vec::new();
    for (index, &category) in categories.iter().enumerate() {
        if index > 0 {
            lines.push(Line::raw(""));
        }
        let muscles: Vec<(Muscle, Sets)> = category
            .muscles()
            .map(|muscle| (muscle, app.tracker.log.sets(app.week, muscle)))
            .collect();
        lines.push(category_header(category, &muscles, target, theme));
        for (muscle, sets) in muscles {
            rows.push((lines.len(), muscle));
            let row = if muscle == app.selected {
                selected = Some(lines.len());
                Row::Selected
            } else {
                Row::Plain
            };
            lines.push(muscle_row(muscle, sets, target, row, width, theme));
        }
    }
    CategoryLines {
        lines,
        selected,
        muscles: rows,
    }
}

fn category_header(
    category: Category,
    muscles: &[(Muscle, Sets)],
    target: Target,
    theme: Theme,
) -> Line<'static> {
    let total: Sets = muscles.iter().map(|&(_, sets)| sets).sum();
    let on_target = muscles
        .iter()
        .filter(|&&(_, sets)| sets >= target.sets())
        .count();
    Line::from(vec![
        Span::styled(category.name().to_uppercase(), theme.category(category)),
        Span::styled(
            format!("  {total} sets  ·  {on_target}/{} on target", muscles.len()),
            theme.dim(),
        ),
    ])
}

fn muscle_row(
    muscle: Muscle,
    sets: Sets,
    target: Target,
    row: Row,
    width: u16,
    theme: Theme,
) -> Line<'static> {
    let fixed = MARKER_WIDTH + u16::try_from(NAME_WIDTH).unwrap_or(14) + COUNT_WIDTH + STATUS_WIDTH;
    let bar_room = width.saturating_sub(fixed + 1);

    let mut spans = vec![
        Span::styled(row.marker(), theme.selected()),
        Span::styled(fit(muscle.name(), NAME_WIDTH), row.style(theme)),
    ];
    if bar_room >= set_bar_width(target, 0) {
        spans.extend(set_bar(sets, target, bar_room, theme));
    }
    let count_style = if sets.is_zero() {
        theme.dim()
    } else {
        theme.bold()
    };
    spans.push(Span::styled(format!(" {sets:>4}"), count_style));
    spans.push(Span::styled(format!(" / {target:<3}"), theme.dim()));
    let status = status_text(sets, target);
    let status_style = if sets >= target.sets() {
        theme.success()
    } else {
        theme.dim()
    };
    spans.push(Span::styled(format!(" {status}"), status_style));
    Line::from(spans)
}
