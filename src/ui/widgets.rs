//! Small building blocks shared by several screens.

use std::cmp::Ordering;

use ratatui::{
    layout::{Constraint, Flex, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
};

use super::theme::Theme;
use crate::{
    app::Fill,
    domain::{ProgressLevel, Sets, Target},
};

/// Whether a list row has the cursor on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row {
    Selected,
    Plain,
}

impl Row {
    /// The `▸` cursor, or matching blank space.
    pub const fn marker(self) -> &'static str {
        match self {
            Self::Selected => "▸ ",
            Self::Plain => "  ",
        }
    }

    pub fn style(self, theme: Theme) -> Style {
        match self {
            Self::Selected => theme.selected(),
            Self::Plain => theme.text(),
        }
    }
}

/// Extra segments drawn past the target, as in the iOS app.
const BAR_BUFFER: u32 = 3;

/// Left-to-right eighth blocks for a partly filled cell, one eighth to seven.
const PARTIAL: [&str; 7] = ["▏", "▎", "▍", "▌", "▋", "▊", "▉"];

/// A segmented progress bar: one segment per set, a `│` after the target.
///
/// Uses two cells per set when `width` allows, otherwise one. Half sets are
/// drawn as a half block. Fill colour shows the progress level, but the
/// numbers beside the bar carry the same information.
pub fn set_bar(sets: Sets, target: Target, width: u16, theme: Theme) -> Vec<Span<'static>> {
    let level = ProgressLevel::of(sets, target);
    fill_bar(Fill::from(sets), level, target, width, theme)
}

/// [`set_bar`] filled to `fill`, which falls between half sets while the bar
/// moves; a partly filled cell is drawn as an eighth block.
pub fn fill_bar(
    fill: Fill,
    level: ProgressLevel,
    target: Target,
    width: u16,
    theme: Theme,
) -> Vec<Span<'static>> {
    let target_sets = u32::from(target.get());
    let segments = target_sets + BAR_BUFFER;
    let available = u32::from(width).saturating_sub(1); // the target marker
    let cells_per_set = if available >= segments * 2 { 2 } else { 1 };
    let per_cell = Fill::PER_SET / cells_per_set;
    let filled_style = theme.level(level);
    let empty = theme.dim();

    // Adjacent cells with the same style share a span.
    let mut spans = Vec::new();
    let mut push = |text: &'static str, style: Style| match spans.last_mut() {
        Some(Span {
            content,
            style: last,
        }) if *last == style => content.to_mut().push_str(text),
        _ => spans.push(Span::styled(text, style)),
    };
    for cell in 0..segments * cells_per_set {
        if cell == target_sets * cells_per_set {
            push("│", theme.bold());
        }
        let filled = fill
            .sixteenths()
            .saturating_sub(cell * per_cell)
            .min(per_cell);
        match filled * 8 / per_cell {
            0 => push("░", empty),
            8 => push("█", filled_style),
            eighths => {
                let glyph = usize::try_from(eighths - 1).map_or("▌", |index| PARTIAL[index]);
                push(glyph, filled_style);
            }
        }
    }
    spans
}

/// Width of [`set_bar`] for a target, given the space available.
pub fn set_bar_width(target: Target, available: u16) -> u16 {
    let segments = u16::from(target.get()) + u16::try_from(BAR_BUFFER).unwrap_or(3);
    if available > segments * 2 {
        segments * 2 + 1
    } else {
        segments + 1
    }
}

/// "3 to go", "✓ on target", "✓ +2.5": progress in words, not just colour.
pub fn status_text(sets: Sets, target: Target) -> String {
    let goal = target.sets();
    match sets.cmp(&goal) {
        Ordering::Less => format!("{} to go", goal.saturating_sub(sets)),
        Ordering::Equal => "✓ on target".to_owned(),
        Ordering::Greater => format!("✓ +{}", sets.saturating_sub(goal)),
    }
}

/// A run of `key description` pairs for the footer or help text.
pub fn key_hints(hints: &[(&str, &str)], theme: Theme) -> Line<'static> {
    let mut spans = Vec::new();
    for (index, (key, action)) in hints.iter().enumerate() {
        if index > 0 {
            spans.push(Span::styled("  ·  ", theme.dim()));
        }
        spans.push(Span::styled((*key).to_owned(), theme.key()));
        spans.push(Span::styled(format!(" {action}"), theme.dim()));
    }
    Line::from(spans)
}

/// Where each span of a left-aligned, one-row line lands in `area`.
///
/// Spans past the right edge get an empty rectangle.
pub fn span_areas(area: Rect, spans: &[Span<'_>]) -> Vec<Rect> {
    let mut x = area.x;
    spans
        .iter()
        .map(|span| {
            let width = u16::try_from(span.width()).unwrap_or(u16::MAX);
            let rect = Rect {
                x,
                width,
                height: 1,
                ..area
            };
            x = x.saturating_add(width);
            rect.intersection(area)
        })
        .collect()
}

/// The row `lines[index]` starts on when the lines wrap at `width`.
pub fn wrapped_row(lines: &[Line<'_>], index: usize, width: u16) -> u16 {
    let before = lines[..index.min(lines.len())].to_vec();
    let rows = Paragraph::new(before)
        .wrap(Wrap { trim: false })
        .line_count(width);
    u16::try_from(rows).unwrap_or(u16::MAX)
}

/// A one-row rectangle `row` rows into `area`, or an empty one below it.
pub fn row_in(area: Rect, row: u16) -> Rect {
    Rect {
        y: area.y.saturating_add(row),
        height: 1,
        ..area
    }
    .intersection(area)
}

/// A rectangle of at most `width` × `height`, centred in `area`.
pub fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let [row] = Layout::vertical([Constraint::Length(height.min(area.height))])
        .flex(Flex::Center)
        .areas(area);
    let [rect] = Layout::horizontal([Constraint::Length(width.min(area.width))])
        .flex(Flex::Center)
        .areas(row);
    rect
}

/// Pads or truncates `text` to exactly `width` columns, ending in `…` when cut.
pub fn fit(text: &str, width: usize) -> String {
    let mut out = String::new();
    let mut used = 0;
    let full = Span::raw(text).width();
    if full <= width {
        out.push_str(text);
        used = full;
    } else if width > 0 {
        for c in text.chars() {
            let w = Span::raw(c.to_string()).width();
            if used + w + 1 > width {
                break;
            }
            out.push(c);
            used += w;
        }
        out.push('…');
        used += 1;
    }
    out.extend(std::iter::repeat_n(' ', width.saturating_sub(used)));
    out
}

/// Sparkline steps, lowest to highest.
const SPARK: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// One glyph per week, oldest first, scaled so a week at or past the
/// target is a full block. Empty weeks are a dim floor, so the shape holds
/// without colour.
pub fn sparkline(weeks: &[Sets], target: Target, theme: Theme) -> Vec<Span<'static>> {
    let goal = target.sets().halves().max(1);
    let top = SPARK.len() - 1;
    weeks
        .iter()
        .map(|sets| {
            if sets.is_zero() {
                return Span::styled(SPARK[0].to_string(), theme.dim());
            }
            // Any sets lift a week off the floor; only the target reaches the top.
            let step = if sets.halves() >= goal {
                top
            } else {
                1 + usize::try_from(sets.halves() * 5 / goal).unwrap_or(0)
            };
            Span::styled(SPARK[step].to_string(), theme.text())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bar(halves: u32, target: u8, width: u16) -> String {
        set_bar(
            Sets::from_halves(halves),
            Target::new(target).unwrap(),
            width,
            Theme::MONO,
        )
        .iter()
        .map(|span| span.content.as_ref())
        .collect()
    }

    #[test]
    fn sparkline_rises_to_the_target() {
        let weeks: Vec<Sets> = [0, 1, 8, 16, 30]
            .into_iter()
            .map(Sets::from_halves)
            .collect();
        let line: String = sparkline(&weeks, Target::new(8).unwrap(), Theme::MONO)
            .iter()
            .map(|span| span.content.as_ref())
            .collect();
        assert_eq!(line, "▁▂▄██");
    }

    #[test]
    fn bar_uses_two_cells_per_set_when_it_fits() {
        assert_eq!(bar(5, 4, 40), "█████░░░│░░░░░░");
    }

    #[test]
    fn bar_falls_back_to_one_cell_per_set() {
        assert_eq!(bar(5, 4, 10), "██▌░│░░░");
        assert_eq!(bar(40, 4, 10), "████│███", "overflow fills the bar");
    }

    #[test]
    fn a_moving_bar_fills_part_of_a_cell() {
        let target = Target::new(4).unwrap();
        let drawn = |sixteenths: u32| -> String {
            fill_bar(
                Fill::from_sixteenths(sixteenths),
                ProgressLevel::Low,
                target,
                10,
                Theme::MONO,
            )
            .iter()
            .map(|span| span.content.as_ref())
            .collect()
        };
        assert_eq!(drawn(16 + 4), "█▎░░│░░░", "a quarter of the second set");
        assert_eq!(drawn(16 + 14), "█▉░░│░░░");
    }

    #[test]
    fn bar_width_matches_the_bar() {
        for width in [8, 10, 20, 40] {
            let target = Target::new(8).unwrap();
            let drawn = bar(3, 8, width).chars().count();
            assert_eq!(
                usize::from(set_bar_width(target, width)),
                drawn,
                "width {width}"
            );
        }
    }

    #[test]
    fn status_reads_without_colour() {
        let target = Target::new(8).unwrap();
        assert_eq!(status_text(Sets::from_halves(9), target), "3.5 to go");
        assert_eq!(status_text(Sets::whole(8), target), "✓ on target");
        assert_eq!(status_text(Sets::whole(11), target), "✓ +3");
    }

    #[test]
    fn fit_pads_and_truncates_by_display_width() {
        assert_eq!(fit("Chest", 8), "Chest   ");
        assert_eq!(fit("Lateral Delts", 8), "Lateral…");
        assert_eq!(fit("腿部训练", 5), "腿部…");
    }
}
