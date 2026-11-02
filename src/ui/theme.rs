use ratatui::style::{Color, Modifier, Style};

use crate::{
    app::ToastKind,
    domain::{Category, ProgressLevel},
};

/// Every style the UI uses, so colour can be switched off in one place.
///
/// Colours are the 16 ANSI ones, which the user's terminal theme maps to
/// shades that read well on its own background.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    color: Palette,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Palette {
    Ansi,
    Mono,
}

impl Theme {
    pub const COLOR: Self = Self {
        color: Palette::Ansi,
    };
    pub const MONO: Self = Self {
        color: Palette::Mono,
    };

    /// Colour unless `NO_COLOR` is set or the terminal is `dumb`.
    pub fn from_env() -> Self {
        let no_color = std::env::var_os("NO_COLOR").is_some_and(|value| !value.is_empty());
        let dumb = std::env::var("TERM").is_ok_and(|term| term == "dumb");
        if no_color || dumb {
            Self::MONO
        } else {
            Self::COLOR
        }
    }

    fn fg(self, color: Color) -> Style {
        match self.color {
            Palette::Ansi => Style::new().fg(color),
            Palette::Mono => Style::new(),
        }
    }

    #[expect(
        clippy::unused_self,
        reason = "every style is a method, so call sites read alike"
    )]
    pub fn text(self) -> Style {
        Style::new()
    }

    pub fn dim(self) -> Style {
        match self.color {
            Palette::Ansi => Style::new().fg(Color::DarkGray),
            Palette::Mono => Style::new().add_modifier(Modifier::DIM),
        }
    }

    #[expect(
        clippy::unused_self,
        reason = "every style is a method, so call sites read alike"
    )]
    pub fn bold(self) -> Style {
        Style::new().add_modifier(Modifier::BOLD)
    }

    pub fn accent(self) -> Style {
        self.fg(Color::Blue).add_modifier(Modifier::BOLD)
    }

    pub fn heading(self) -> Style {
        self.bold().add_modifier(Modifier::UNDERLINED)
    }

    pub fn key(self) -> Style {
        self.fg(Color::Blue).add_modifier(Modifier::BOLD)
    }

    pub fn border(self) -> Style {
        self.dim()
    }

    pub fn focus_border(self) -> Style {
        self.fg(Color::Blue)
    }

    /// The highlighted row of a list.
    pub fn selected(self) -> Style {
        match self.color {
            Palette::Ansi => Style::new().fg(Color::Blue).add_modifier(Modifier::BOLD),
            Palette::Mono => Style::new().add_modifier(Modifier::REVERSED),
        }
    }

    /// Laid over whatever is under the pointer. Underline reads the same with
    /// or without colour and leaves each part's own colours alone.
    #[expect(
        clippy::unused_self,
        reason = "every style is a method, so call sites read alike"
    )]
    pub fn hover(self) -> Style {
        Style::new().add_modifier(Modifier::UNDERLINED)
    }

    #[expect(
        clippy::unused_self,
        reason = "every style is a method, so call sites read alike"
    )]
    pub fn tab_active(self) -> Style {
        Style::new().add_modifier(Modifier::BOLD | Modifier::REVERSED)
    }

    /// Letters of a name that matched the search. Bold and underlined read
    /// on any row, selected or not, with or without colour.
    #[expect(
        clippy::unused_self,
        reason = "every style is a method, so call sites read alike"
    )]
    pub fn matched(self) -> Style {
        Style::new().add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
    }

    pub fn success(self) -> Style {
        self.fg(Color::Green)
    }

    /// A brief flash on a row that just reached its target. Reversed video
    /// shows without colour too; the ✓ beside it says the same thing.
    pub fn pulse(self) -> Style {
        self.success()
            .add_modifier(Modifier::BOLD | Modifier::REVERSED)
    }

    pub fn error(self) -> Style {
        self.fg(Color::Red).add_modifier(Modifier::BOLD)
    }

    pub fn toast(self, kind: ToastKind) -> Style {
        match kind {
            ToastKind::Info => self.accent(),
            ToastKind::Success => self.success().add_modifier(Modifier::BOLD),
            ToastKind::Error => self.error(),
        }
    }

    pub fn category(self, category: Category) -> Style {
        let color = match category {
            Category::Push => Color::Cyan,
            Category::Pull => Color::Yellow,
            Category::Legs => Color::Magenta,
            Category::Core => Color::Blue,
        };
        self.fg(color).add_modifier(Modifier::BOLD)
    }

    /// Whether colour alone can tell levels apart; without it, glyphs must.
    pub fn has_color(self) -> bool {
        self.color == Palette::Ansi
    }

    /// Progress colours follow the iOS app: red, orange, then on-target hues.
    pub fn level(self, level: ProgressLevel) -> Style {
        self.fg(match level {
            ProgressLevel::Low => Color::Red,
            ProgressLevel::Medium => Color::Yellow,
            ProgressLevel::Good => Color::Green,
            ProgressLevel::Excellent => Color::Cyan,
            ProgressLevel::Overkill => Color::Magenta,
        })
    }
}
