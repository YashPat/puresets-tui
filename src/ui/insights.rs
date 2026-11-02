//! Insights: averages over a period, per category and per muscle, plus a
//! history of weekly totals.

use std::fmt::Write as _;

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Bar, BarChart, BarGroup, Block, Borders, Padding, Paragraph},
};

use super::{
    HitMap,
    theme::Theme,
    widgets::{fit, set_bar, set_bar_width, span_areas, sparkline, status_text},
};
use crate::{
    app::{App, MouseTarget},
    domain::{CategorySummary, Insights, Period, ProgressLevel, Sets, Target, muscle_week_levels},
};

const NAME: usize = 14;
const HISTORY_WEEKS: usize = 12;
/// Weeks in each muscle's trend sparkline.
const TREND_WEEKS: usize = 8;
const HISTORY_MIN_WIDTH: u16 = 50;
/// Columns of the muscle x week heatmap.
const HEATMAP_WEEKS: usize = 12;
/// Each week is this many identical glyphs wide.
const HEATMAP_CELL: usize = 2;
/// Heading, one row per muscle, week axis and legend.
const HEATMAP_HEIGHT: u16 = 18;
/// The sets-per-week chart keeps at least this much beside the heatmap.
const HISTORY_MIN_HEIGHT: u16 = 9;
const CARD_HEIGHT: u16 = 6;
/// Width needed for the four category cards in a single row.
const CARDS_IN_ROW: u16 = 96;

pub fn draw(frame: &mut Frame<'_>, area: Rect, app: &App, hits: &mut HitMap, theme: Theme) {
    let target = app.target();
    let current = app.current_week();
    let insights = Insights::compute(&app.tracker.log, current, app.period, target);

    let card_rows: u16 = if area.width >= CARDS_IN_ROW { 1 } else { 2 };
    let [selector, _, cards, _, body] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(CARD_HEIGHT * card_rows),
        Constraint::Length(1),
        Constraint::Min(0),
    ])
    .areas(area);

    draw_selector(frame, selector, app.period, &insights, theme);
    // Each period's label is followed by a spacer span.
    let labels = span_areas(selector, &period_spans(app.period, theme));
    for (period, label) in Period::ALL.into_iter().zip(labels.iter().step_by(2)) {
        hits.add(*label, MouseTarget::Period(period));
    }
    draw_cards(frame, cards, &insights, target, card_rows, theme);

    if body.width >= HISTORY_MIN_WIDTH * 2 {
        let [muscles, _, history] = Layout::horizontal([
            Constraint::Fill(3),
            Constraint::Length(3),
            Constraint::Fill(2),
        ])
        .areas(body);
        draw_ranked(frame, muscles, app, &insights, theme);
        let (history, heatmap) = split_heatmap(history);
        draw_history(frame, history, app, theme);
        if let Some(heatmap) = heatmap {
            draw_heatmap(frame, heatmap, app, theme);
        }
    } else {
        draw_ranked(frame, body, app, &insights, theme);
    }
}

fn draw_selector(
    frame: &mut Frame<'_>,
    area: Rect,
    period: Period,
    insights: &Insights,
    theme: Theme,
) {
    let mut spans = period_spans(period, theme);
    let (Some(newest), Some(oldest)) = (insights.weeks.first(), insights.weeks.last()) else {
        frame.render_widget(Line::from(spans), area);
        return;
    };
    let range = if newest == oldest {
        newest.range_label()
    } else {
        format!(
            "{} – {}",
            oldest.monday().strftime("%b %-d, %Y"),
            newest.sunday().strftime("%b %-d, %Y")
        )
    };
    spans.push(Span::styled(format!("  {range}  ·  "), theme.dim()));
    spans.push(Span::styled(insights.total.to_string(), theme.bold()));
    spans.push(Span::styled(" sets logged", theme.dim()));
    frame.render_widget(Line::from(spans), area);
}

/// The period choices, each a label then a space.
fn period_spans(period: Period, theme: Theme) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    for option in Period::ALL {
        let style = if option == period {
            theme.tab_active()
        } else {
            theme.dim()
        };
        spans.push(Span::styled(format!(" {} ", option.label()), style));
        spans.push(Span::raw(" "));
    }
    spans
}

fn draw_cards(
    frame: &mut Frame<'_>,
    area: Rect,
    insights: &Insights,
    target: Target,
    rows: u16,
    theme: Theme,
) {
    let row_areas =
        Layout::vertical(vec![Constraint::Length(CARD_HEIGHT); usize::from(rows)]).split(area);
    let per_row = insights.categories.len() / usize::from(rows);
    for (row, chunk) in row_areas
        .iter()
        .zip(insights.categories.chunks(per_row.max(1)))
    {
        let cells = Layout::horizontal(vec![Constraint::Fill(1); chunk.len()])
            .spacing(2)
            .split(*row);
        for (cell, summary) in cells.iter().zip(chunk) {
            draw_card(frame, *cell, summary, target, theme);
        }
    }
}

fn draw_card(
    frame: &mut Frame<'_>,
    area: Rect,
    summary: &CategorySummary,
    target: Target,
    theme: Theme,
) {
    let block = Block::new()
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(theme.border())
        .padding(Padding::horizontal(1))
        .title(Span::styled(
            format!(" {} ", summary.category.name()),
            theme.category(summary.category),
        ));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let width = usize::from(inner.width);
    let filled = width * usize::from(summary.completion.min(100)) / 100;
    let level_style = theme.level(ProgressLevel::of(summary.average, target));
    let below = match summary.below_target {
        0 => Span::styled("✓ all on target", theme.success()),
        1 => Span::styled("1 muscle below target", theme.dim()),
        n => Span::styled(format!("{n} muscles below target"), theme.dim()),
    };
    let lines = vec![
        Line::from(vec![
            Span::styled(summary.average.to_string(), theme.bold()),
            Span::styled(" avg/muscle  ·  ", theme.dim()),
            Span::styled(summary.total.to_string(), theme.bold()),
            Span::styled(" total", theme.dim()),
        ]),
        Line::from(vec![
            Span::styled("█".repeat(filled), level_style),
            Span::styled("░".repeat(width.saturating_sub(filled)), theme.dim()),
        ]),
        Line::from(vec![
            Span::styled(format!("{}%", summary.completion), theme.bold()),
            Span::styled(" of target", theme.dim()),
        ]),
        Line::from(below),
    ];
    frame.render_widget(Paragraph::new(lines), inner);
}

fn draw_ranked(frame: &mut Frame<'_>, area: Rect, app: &App, insights: &Insights, theme: Theme) {
    let target = app.target();
    let heading = if insights.weeks.len() > 1 {
        "AVERAGE SETS PER WEEK"
    } else {
        "SETS THIS WEEK"
    };
    let mut lines = vec![Line::styled(heading, theme.heading())];

    // Name, count ("12.5 "), status ("✓ on target").
    let fixed = u16::try_from(NAME).unwrap_or(14) + 6 + 12;
    let mut bar_room = area.width.saturating_sub(fixed);
    let show_bar = bar_room >= set_bar_width(target, 0);
    // The trend takes room only when the bar still fits beside it.
    let trend_width = u16::try_from(TREND_WEEKS + 1).unwrap_or(9);
    let show_trend = bar_room >= set_bar_width(target, 0) + trend_width;
    let trend_weeks: Vec<_> = app.current_week().trailing(TREND_WEEKS).collect();
    if insights.total.is_zero() {
        lines.push(Line::raw(""));
        lines.push(Line::styled(
            "Nothing logged in this period yet. Log sets from Home with enter.",
            theme.dim(),
        ));
    }
    // The line above the rows labels the trend column, or stays blank.
    if show_trend {
        bar_room -= trend_width;
        lines.push(Line::styled(
            format!("{:NAME$}{TREND_WEEKS} weeks", ""),
            theme.dim(),
        ));
    } else {
        lines.push(Line::raw(""));
    }
    for (muscle, sets) in insights.ranked() {
        let mut spans = vec![Span::styled(fit(muscle.name(), NAME), theme.text())];
        if show_trend {
            let weeks: Vec<Sets> = trend_weeks
                .iter()
                .rev()
                .map(|&week| app.tracker.log.sets(week, muscle))
                .collect();
            spans.extend(sparkline(&weeks, target, theme));
            spans.push(Span::raw(" "));
        }
        if show_bar {
            spans.extend(set_bar(sets, target, bar_room, theme));
        }
        let count_style = if sets.is_zero() {
            theme.dim()
        } else {
            theme.bold()
        };
        spans.push(Span::styled(format!(" {sets:>4} "), count_style));
        let status_style = if sets >= target.sets() {
            theme.success()
        } else {
            theme.dim()
        };
        spans.push(Span::styled(status_text(sets, target), status_style));
        lines.push(Line::from(spans));
    }
    frame.render_widget(Paragraph::new(lines), area);
}

fn draw_history(frame: &mut Frame<'_>, area: Rect, app: &App, theme: Theme) {
    let weeks: Vec<_> = app.current_week().trailing(HISTORY_WEEKS).collect();
    let bars: Vec<Bar<'_>> = weeks
        .iter()
        .rev()
        .map(|&week| {
            let total = app.tracker.log.week_total(week);
            let style: Style = if week == app.current_week() {
                theme.accent()
            } else {
                theme.text()
            };
            Bar::default()
                .value(u64::from(total.halves() / 2))
                .text_value(String::new())
                .label(Line::from(format!("{}", week.number())))
                .style(style)
        })
        .collect();

    let [heading, _, values, chart, caption] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .areas(area);
    frame.render_widget(Line::styled("SETS PER WEEK", theme.heading()), heading);

    let count = u16::try_from(bars.len()).unwrap_or(1).max(1);
    let bar_width = (chart.width.saturating_sub(count) / count).clamp(1, 5);
    let chart_widget = BarChart::default()
        .data(BarGroup::default().bars(&bars))
        .bar_width(bar_width)
        .bar_gap(1)
        .bar_style(theme.text());
    frame.render_widget(chart_widget, chart);

    // Totals sit in a row above the bars rather than painted onto them, where
    // they were unreadable and clipped to the bar width.
    let slot = usize::from(bar_width) + 1;
    let mut row = String::new();
    for &week in weeks.iter().rev() {
        let total = app.tracker.log.week_total(week);
        let mut text = total.to_string();
        if text.len() > slot - 1 {
            text = (total.halves() / 2).to_string();
        }
        if text.len() > slot - 1 {
            text.clear();
        }
        let _ = write!(row, "{text:^w$} ", w = slot - 1);
    }
    frame.render_widget(Line::styled(row, theme.dim()), values);
    frame.render_widget(
        Line::styled(
            format!("last {HISTORY_WEEKS} weeks, by week number"),
            theme.dim(),
        ),
        caption,
    );
}

/// Room for the heatmap below the history chart, when the column is tall and
/// wide enough. At 80x24 the screen already holds cards and the ranked list,
/// so it is left out rather than squeezed.
fn split_heatmap(area: Rect) -> (Rect, Option<Rect>) {
    let width = u16::try_from(NAME + 1 + HEATMAP_WEEKS * HEATMAP_CELL).unwrap_or(u16::MAX);
    if area.width < width || area.height < HEATMAP_HEIGHT + 1 + HISTORY_MIN_HEIGHT {
        return (area, None);
    }
    let [history, _, heatmap] = Layout::vertical([
        Constraint::Min(HISTORY_MIN_HEIGHT),
        Constraint::Length(1),
        Constraint::Length(HEATMAP_HEIGHT),
    ])
    .areas(area);
    (history, Some(heatmap))
}

/// Solid blocks when colour carries the level; otherwise one shade per level,
/// so the grid still reads under `NO_COLOR`. `None` is a week before tracking
/// began.
fn heat_glyph(level: Option<ProgressLevel>, theme: Theme) -> char {
    if theme.has_color() && level.is_some() {
        return '█';
    }
    match level {
        None => '·',
        Some(ProgressLevel::Low) => '░',
        Some(ProgressLevel::Medium) => '▒',
        Some(ProgressLevel::Good) => '▓',
        Some(ProgressLevel::Excellent) => '█',
        Some(ProgressLevel::Overkill) => '◆',
    }
}

fn heat_style(level: Option<ProgressLevel>, theme: Theme) -> Style {
    level.map_or_else(|| theme.dim(), |level| theme.level(level))
}

fn heat_span(level: Option<ProgressLevel>, theme: Theme) -> Span<'static> {
    Span::styled(
        heat_glyph(level, theme).to_string().repeat(HEATMAP_CELL),
        heat_style(level, theme),
    )
}

/// Muscles down, the last twelve weeks across, each cell shaded by how that
/// week compared with the target.
fn draw_heatmap(frame: &mut Frame<'_>, area: Rect, app: &App, theme: Theme) {
    let current = app.current_week();
    let rows = muscle_week_levels(
        &app.tracker.log,
        Some(app.earliest_week()),
        current,
        HEATMAP_WEEKS,
        app.target(),
    );
    let mut lines = vec![Line::styled("MUSCLES BY WEEK", theme.heading())];
    for (muscle, levels) in rows {
        let mut spans = vec![Span::styled(fit(muscle.name(), NAME + 1), theme.text())];
        spans.extend(levels.into_iter().map(|level| heat_span(level, theme)));
        lines.push(Line::from(spans));
    }

    // Week numbers under every other column, ending on the newest week.
    let step = HEATMAP_CELL * 2;
    let mut labels: Vec<String> = current
        .trailing(HEATMAP_WEEKS)
        .step_by(2)
        .map(|week| format!("{:<step$}", week.number()))
        .collect();
    labels.reverse();
    // The oldest label sits over the column of its week, not the first one.
    let offset = (HEATMAP_WEEKS - 1) % 2 * HEATMAP_CELL;
    let axis = format!(
        "{:width$}{}",
        "",
        labels.concat(),
        width = NAME + 1 + offset
    );
    lines.push(Line::styled(axis.trim_end().to_string(), theme.dim()));

    let mut legend: Vec<Span<'static>> = Vec::new();
    for (level, text) in [
        (Some(ProgressLevel::Low), "<50% "),
        (Some(ProgressLevel::Medium), "50 "),
        (Some(ProgressLevel::Good), "75 "),
        (Some(ProgressLevel::Excellent), "100 "),
        (Some(ProgressLevel::Overkill), "125+ "),
        (None, "none"),
    ] {
        legend.push(Span::styled(
            heat_glyph(level, theme).to_string(),
            heat_style(level, theme),
        ));
        legend.push(Span::styled(format!(" {text}"), theme.dim()));
    }
    lines.push(Line::from(legend));
    frame.render_widget(Paragraph::new(lines), area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Muscle;

    #[test]
    fn heatmap_height_fits_every_muscle() {
        assert_eq!(usize::from(HEATMAP_HEIGHT), Muscle::ALL.len() + 3);
    }
}
