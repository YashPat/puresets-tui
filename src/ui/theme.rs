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

    #[expect(
        clippy::unused_self,
        reason = "every style is a method, so call sites read alike"
    )]
    pub fn tab_active(self) -> Style {
        Style::new().add_modifier(Modifier::BOLD | Modifier::REVERSED)
    }

    pub fn success(self) -> Style {
        self.fg(Color::Green)
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
