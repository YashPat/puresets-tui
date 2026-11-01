//! Settings: the weekly target and where the data lives.

use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
};

use super::theme::Theme;
use crate::{
    app::App,
    domain::{Goal, Target},
};

const LABEL_WIDTH: usize = 18;

pub fn draw(frame: &mut Frame<'_>, area: Rect, app: &App, theme: Theme) {
    let target = app.target();
    let label = |text: &str| Span::styled(format!("{text:<LABEL_WIDTH$}"), theme.dim());
    let preset = target.goal().map_or_else(
        || Span::styled("custom", theme.dim()),
        |goal| Span::styled(goal.name(), theme.success()),
    );
    let at_min = target.get() == Target::MIN;
    let at_max = target.get() == Target::MAX;
    let arrow = |glyph: &'static str, disabled: bool| {
        Span::styled(glyph, if disabled { theme.dim() } else { theme.key() })
    };

    let mut lines = vec![
        Line::styled("WEEKLY TARGET", theme.heading()),
        Line::raw(""),
        Line::from(vec![
            label("Sets per muscle"),
            arrow("‹ ", at_min),
            Span::styled(format!("{target:>2}"), theme.bold()),
            arrow(" ›", at_max),
            Span::raw("   "),
            preset,
        ]),
        Line::from(vec![
            Span::raw(" ".repeat(LABEL_WIDTH)),
            Span::styled(
                format!("h/l to change, {} to {}", Target::MIN, Target::MAX),
                theme.dim(),
            ),
        ]),
        Line::raw(""),
    ];
    for (key, goal) in [("m", Goal::Moderate), ("o", Goal::Optimal)] {
        let active = target.goal() == Some(goal);
        lines.push(Line::from(vec![
            Span::raw(" ".repeat(LABEL_WIDTH)),
            Span::styled(format!("{key} "), theme.key()),
            Span::styled(
                format!("{:<9}", goal.name()),
                if active {
                    theme.selected()
                } else {
                    theme.bold()
                },
            ),
            Span::styled(format!("{:>2} sets  ", goal.target()), theme.text()),
            Span::styled(goal.blurb(), theme.dim()),
        ]));
    }

    let customs = app.tracker.catalog.customs().count();
    let since = app.tracker.first_week.map_or_else(
        || "not yet".to_owned(),
        |week| {
            format!(
                "Week {} of {} ({})",
                week.number(),
                week.year(),
                week.range_label()
            )
        },
    );
    lines.extend([
        Line::raw(""),
        Line::raw(""),
        Line::styled("YOUR DATA", theme.heading()),
        Line::raw(""),
        Line::from(vec![
            label("Data file"),
            Span::styled(app.data_path.clone(), theme.text()),
        ]),
        Line::from(vec![
            label("Tracking since"),
            Span::styled(since, theme.text()),
        ]),
        Line::from(vec![
            label("Weeks logged"),
            Span::styled(app.tracker.log.weeks().len().to_string(), theme.text()),
        ]),
        Line::from(vec![
            label("Exercises"),
            Span::styled(
                format!(
                    "{} built in, {customs} custom",
                    app.tracker.catalog.len() - customs
                ),
                theme.text(),
            ),
        ]),
        Line::raw(""),
        Line::styled(
            "Changes save automatically. The file is plain JSON: back it up or edit it \
             while PureSets is closed. Set PURESETS_DATA or pass --data to use another file.",
            theme.dim(),
        ),
    ]);
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
}
