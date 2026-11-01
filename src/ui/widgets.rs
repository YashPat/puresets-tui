//! Small building blocks shared by several screens.

use std::cmp::Ordering;

use ratatui::{
    layout::{Constraint, Flex, Layout, Rect},
    style::Style,
    text::{Line, Span},
};

use super::theme::Theme;
use crate::domain::{ProgressLevel, Sets, Target};

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

/// A segmented progress bar: one segment per set, a `│` after the target.
///
/// Uses two cells per set when `width` allows, otherwise one. Half sets are
/// drawn as a half block. Fill colour shows the progress level, but the
/// numbers beside the bar carry the same information.
pub fn set_bar(sets: Sets, target: Target, width: u16, theme: Theme) -> Vec<Span<'static>> {
    let target_sets = u32::from(target.get());
    let segments = target_sets + BAR_BUFFER;
    let available = u32::from(width).saturating_sub(1); // the target marker
    let cells_per_set = if available >= segments * 2 { 2 } else { 1 };
    let fill = theme.level(ProgressLevel::of(sets, target));
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
    let halves = sets.halves();
    for segment in 0..segments {
        if segment == target_sets {
            push("│", theme.bold());
        }
        let filled_halves = halves.saturating_sub(segment * 2).min(2);
        match (cells_per_set, filled_halves) {
            (2, 2) => push("██", fill),
            (2, 1) => {
                push("█", fill);
                push("░", empty);
            }
            (2, _) => push("░░", empty),
            (_, 2) => push("█", fill),
            (_, 1) => push("▌", fill),
            _ => push("░", empty),
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
    fn bar_uses_two_cells_per_set_when_it_fits() {
        assert_eq!(bar(5, 4, 40), "█████░░░│░░░░░░");
    }

    #[test]
    fn bar_falls_back_to_one_cell_per_set() {
        assert_eq!(bar(5, 4, 10), "██▌░│░░░");
        assert_eq!(bar(40, 4, 10), "████│███", "overflow fills the bar");
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
