//! Why: the idea behind `PureSets` and the research curve that inspired it.

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    symbols::Marker,
    text::{Line, Span},
    widgets::{Axis, Chart, Dataset, GraphType, Paragraph, Wrap},
};

use super::theme::Theme;
use crate::{app::App, domain::dose_response};

const SOURCE_URL: &str = "https://link.springer.com/10.1007/s40279-025-02344-w";
/// Width at which the text and chart sit side by side.
const SIDE_BY_SIDE: u16 = 110;
const TEXT_WIDTH: u16 = 64;
const CHART_HEIGHT: u16 = 18;
const CHART_MAX_SETS: f64 = 30.0;

pub fn draw(frame: &mut Frame<'_>, area: Rect, app: &mut App, theme: Theme) {
    let text = philosophy(theme);
    if area.width >= SIDE_BY_SIDE {
        let [left, _, right] = Layout::horizontal([
            Constraint::Length(TEXT_WIDTH),
            Constraint::Length(4),
            Constraint::Min(0),
        ])
        .areas(area);
        draw_text(frame, left, text, app, theme);
        let [chart, _] =
            Layout::vertical([Constraint::Length(CHART_HEIGHT + 2), Constraint::Min(0)])
                .areas(right);
        draw_chart(frame, chart, app, theme);
    } else {
        // Narrow: the chart goes first so it is visible without scrolling.
        let [chart, _, rest] = Layout::vertical([
            Constraint::Length(CHART_HEIGHT.min(area.height / 2)),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .areas(area);
        draw_chart(frame, chart, app, theme);
        draw_text(frame, rest, text, app, theme);
    }
}

fn draw_text(
    frame: &mut Frame<'_>,
    area: Rect,
    text: Vec<Line<'static>>,
    app: &mut App,
    theme: Theme,
) {
    let paragraph = Paragraph::new(text).wrap(Wrap { trim: false });
    let total = u16::try_from(paragraph.line_count(area.width)).unwrap_or(u16::MAX);
    let max_scroll = total.saturating_sub(area.height);
    app.why_scroll = app.why_scroll.min(max_scroll);
    frame.render_widget(paragraph.scroll((app.why_scroll, 0)), area);
    if max_scroll > 0 && app.why_scroll < max_scroll && area.height > 0 {
        let more = Rect {
            y: area.bottom().saturating_sub(1),
            height: 1,
            ..area
        };
        frame.render_widget(
            Line::styled("↓ more (j/k to scroll)", theme.dim()).right_aligned(),
            more,
        );
    }
}

fn philosophy(theme: Theme) -> Vec<Line<'static>> {
    let heading = |text: &'static str| Line::styled(text, theme.heading());
    let body = |text: &'static str| Line::styled(text, theme.text());
    let blank = || Line::raw("");
    let step = |number: &'static str, text: &'static str| {
        Line::from(vec![Span::styled(number, theme.key()), Span::raw(text)])
    };
    vec![
        heading("Simple enough to keep doing"),
        blank(),
        body(
            "Most training logs ask for weight, reps, rest and effort on every set. \
             That detail is useful, but it is also why so many people stop logging. \
             PureSets tracks one thing: how many hard sets each muscle got this week.",
        ),
        blank(),
        body("Simplicity enables consistency, and consistency is what builds muscle."),
        blank(),
        heading("What counts as a hard set"),
        blank(),
        body(
            "A set taken close to failure: the last few reps are a real struggle. \
             Warm-ups and easy sets do not count.",
        ),
        blank(),
        heading("The research"),
        blank(),
        body(
            "A 2025 meta-regression of resistance-training studies modelled muscle growth \
             against weekly sets per muscle. Growth keeps rising with volume, but each \
             extra set adds less: the curve flattens past roughly 12 sets a week.",
        ),
        blank(),
        body(
            "PureSets is inspired by this research, not trying to prescribe. \
             Pick a target you can sustain and recover from.",
        ),
        blank(),
        Line::from(vec![
            Span::styled("Source  ", theme.dim()),
            Span::styled(SOURCE_URL, theme.accent()),
        ]),
        blank(),
        heading("How logging works"),
        blank(),
        step("1  ", "Press enter on Home (or a to search everything)."),
        step("2  ", "Type to find the exercise you did."),
        step("3  ", "Press enter, then choose how many hard sets."),
        step(
            "4  ",
            "Press enter to log. Every muscle it works is credited.",
        ),
        step("5  ", "Check Home for what still needs work this week."),
        blank(),
        body(
            "Primary movers get one set per set performed. Secondary movers get half, \
             because they help but are not the main target.",
        ),
    ]
}

fn draw_chart(frame: &mut Frame<'_>, area: Rect, app: &App, theme: Theme) {
    let points = dose_response();
    let visible = |p: &&crate::domain::DosePoint| p.sets <= CHART_MAX_SETS;
    let gain: Vec<(f64, f64)> = points
        .iter()
        .filter(visible)
        .map(|p| (p.sets, p.gain))
        .collect();
    let lower: Vec<(f64, f64)> = points
        .iter()
        .filter(visible)
        .map(|p| (p.sets, p.lower))
        .collect();
    let upper: Vec<(f64, f64)> = points
        .iter()
        .filter(visible)
        .map(|p| (p.sets, p.upper))
        .collect();
    let max_gain = upper.iter().map(|&(_, y)| y).fold(0.0_f64, f64::max).ceil();

    let target = f64::from(app.target().get());
    let marker = vec![(target, 0.0), (target, max_gain)];

    let datasets = vec![
        Dataset::default()
            .marker(Marker::Braille)
            .graph_type(GraphType::Line)
            .style(theme.dim())
            .data(&upper),
        Dataset::default()
            .marker(Marker::Braille)
            .graph_type(GraphType::Line)
            .style(theme.dim())
            .data(&lower),
        Dataset::default()
            .name(format!("your target ({})", app.target()))
            .marker(Marker::Braille)
            .graph_type(GraphType::Line)
            .style(theme.success())
            .data(&marker),
        Dataset::default()
            .name("expected growth")
            .marker(Marker::Braille)
            .graph_type(GraphType::Line)
            .style(theme.accent())
            .data(&gain),
    ];

    let x_labels: Vec<Span<'static>> = ["0", "10", "20", "30"]
        .into_iter()
        .map(|label| Span::styled(label, theme.dim()))
        .collect();
    let y_labels = vec![
        Span::styled("0%", theme.dim()),
        Span::styled(format!("{}%", max_gain / 2.0), theme.dim()),
        Span::styled(format!("{max_gain}%"), theme.dim()),
    ];

    let [title, chart, caption] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .areas(area);
    frame.render_widget(
        Line::styled("Muscle growth vs weekly sets", theme.heading()),
        title,
    );
    let chart_widget = Chart::new(datasets)
        .x_axis(
            Axis::default()
                .title(Span::styled("sets/week", theme.dim()))
                .bounds([0.0, CHART_MAX_SETS])
                .labels(x_labels)
                .style(theme.border()),
        )
        .y_axis(
            Axis::default()
                .bounds([0.0, max_gain])
                .labels(y_labels)
                .style(theme.border()),
        )
        .legend_position(Some(ratatui::widgets::LegendPosition::BottomRight));
    frame.render_widget(chart_widget, chart);
    frame.render_widget(
        Line::styled("faint lines: 95% credible interval", theme.dim()),
        caption,
    );
}
