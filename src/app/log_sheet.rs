//! The "Log sets" sheet: pick an exercise, choose a set count, log it.
//! Custom exercises are created and deleted from here too.

use ratatui::{
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    widgets::ListState,
};

use super::{MouseAction, MouseTarget, text_input::TextInput};
use crate::domain::{Category, Exercise, ExerciseDraft, Muscle, Origin, Role, Tracker};

/// The most sets that can be logged at once, as in the iOS app.
pub const MAX_SETS: u32 = 20;
const NAME_LIMIT: usize = 48;
const NOTE_LIMIT: usize = 160;
const PAGE: usize = 10;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogSheet {
    pub query: TextInput,
    pub filter: Option<Muscle>,
    pub selected: usize,
    /// Scroll position of the exercise list, kept between frames.
    pub list: ListState,
    pub step: Step,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    Pick,
    Sets(SetCount),
    Note(NoteEditor),
    Create(CreateForm),
    ConfirmDelete { exercise: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetCount {
    pub exercise: String,
    pub count: u32,
    /// Whether the last key was a digit, so the next one extends the number.
    typing: bool,
}

impl SetCount {
    fn new(exercise: String, count: u32) -> Self {
        Self {
            exercise,
            count,
            typing: false,
        }
    }

    fn step_by(&mut self, delta: i32) {
        self.typing = false;
        self.count = self.count.saturating_add_signed(delta).clamp(1, MAX_SETS);
    }

    fn type_digit(&mut self, digit: u32) {
        let extended = self.count * 10 + digit;
        self.count = if self.typing && extended <= MAX_SETS {
            extended
        } else if digit == 0 {
            // A zero cannot start a count, so it leaves the count as it was.
            self.count
        } else {
            digit
        };
        self.typing = true;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteEditor {
    pub exercise: String,
    pub input: TextInput,
    count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Name,
    Muscles,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateForm {
    pub name: TextInput,
    pub roles: [Option<Role>; Muscle::ALL.len()],
    pub focus: Field,
    pub cursor: Muscle,
    pub error: Option<String>,
}

impl CreateForm {
    fn new(name: &str) -> Self {
        Self {
            name: TextInput::new(NAME_LIMIT).with_value(name.trim()),
            roles: [None; Muscle::ALL.len()],
            focus: Field::Name,
            cursor: Muscle::Chest,
            error: None,
        }
    }

    pub fn role(&self, muscle: Muscle) -> Option<Role> {
        self.roles[muscle.index()]
    }

    fn set_role(&mut self, role: Option<Role>) {
        self.roles[self.cursor.index()] = role;
        self.error = None;
    }

    fn cycle_role(&mut self) {
        let next = match self.role(self.cursor) {
            None => Some(Role::Primary),
            Some(Role::Primary) => Some(Role::Secondary),
            Some(Role::Secondary) => None,
        };
        self.set_role(next);
    }

    fn muscles_with(&self, role: Role) -> Vec<Muscle> {
        Muscle::ALL
            .into_iter()
            .filter(|&muscle| self.role(muscle) == Some(role))
            .collect()
    }

    fn draft(&self) -> ExerciseDraft {
        ExerciseDraft {
            name: self.name.value().to_owned(),
            primary: self.muscles_with(Role::Primary),
            secondary: self.muscles_with(Role::Secondary),
            note: String::new(),
        }
    }

    fn handle_key(&mut self, key: KeyEvent, tracker: &mut Tracker) -> FormOutcome {
        match key.code {
            KeyCode::Esc => return FormOutcome::Back,
            KeyCode::Enter => {
                return match tracker.catalog.add_custom(self.draft()) {
                    Ok(name) => FormOutcome::Created(name),
                    Err(err) => {
                        self.error = Some(err.to_string());
                        FormOutcome::Stay
                    }
                };
            }
            KeyCode::Tab | KeyCode::BackTab => {
                self.focus = match self.focus {
                    Field::Name => Field::Muscles,
                    Field::Muscles => Field::Name,
                };
            }
            KeyCode::Down if self.focus == Field::Name => self.focus = Field::Muscles,
            _ if self.focus == Field::Name => {
                if self.name.handle_key(key) {
                    self.error = None;
                }
            }
            KeyCode::Char(' ') => self.cycle_role(),
            KeyCode::Char('p' | '1') => self.set_role(Some(Role::Primary)),
            KeyCode::Char('s' | '2') => self.set_role(Some(Role::Secondary)),
            KeyCode::Char('x' | '0') | KeyCode::Backspace | KeyCode::Delete => self.set_role(None),
            code => self.move_cursor(code),
        }
        FormOutcome::Stay
    }

    /// Moves the grid cursor; the grid has one column per category.
    fn move_cursor(&mut self, key: KeyCode) {
        let category = self.cursor.category();
        let column: Vec<Muscle> = category.muscles().collect();
        let row = column.iter().position(|&m| m == self.cursor).unwrap_or(0);
        let col = Category::ALL
            .iter()
            .position(|&c| c == category)
            .unwrap_or(0);
        let in_column = |col: usize, row: usize| {
            let muscles: Vec<Muscle> = Category::ALL[col].muscles().collect();
            muscles[row.min(muscles.len() - 1)]
        };
        match key {
            KeyCode::Up | KeyCode::Char('k') if row == 0 => self.focus = Field::Name,
            KeyCode::Up | KeyCode::Char('k') => self.cursor = column[row - 1],
            KeyCode::Down | KeyCode::Char('j') if row + 1 < column.len() => {
                self.cursor = column[row + 1];
            }
            KeyCode::Left | KeyCode::Char('h') if col > 0 => self.cursor = in_column(col - 1, row),
            KeyCode::Right | KeyCode::Char('l') if col + 1 < Category::ALL.len() => {
                self.cursor = in_column(col + 1, row);
            }
            _ => {}
        }
    }
}

enum FormOutcome {
    Stay,
    Back,
    Created(String),
}

/// One selectable row of the exercise list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickRow<'a> {
    Exercise {
        exercise: &'a Exercise,
        section: Section,
    },
    NewCustom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Recent,
    All,
    Matches,
}

impl Section {
    pub const fn title(self) -> &'static str {
        match self {
            Self::Recent => "RECENT",
            Self::All => "ALL EXERCISES",
            Self::Matches => "MATCHES",
        }
    }
}

/// What the app should do after the sheet handled a key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SheetEvent {
    None,
    Close,
    /// Log `count` sets of `exercise` and close the sheet.
    Log {
        exercise: String,
        count: u32,
    },
    /// The catalog changed; save and confirm with this message.
    Saved(String),
    /// Tell the user something without changing data.
    Notice(String),
}

impl LogSheet {
    pub fn new(filter: Option<Muscle>) -> Self {
        Self {
            query: TextInput::new(NAME_LIMIT),
            filter,
            selected: 0,
            list: ListState::default(),
            step: Step::Pick,
        }
    }

    /// The exercise list for the current query and filter.
    pub fn rows<'a>(&self, tracker: &'a Tracker) -> Vec<PickRow<'a>> {
        let works = |exercise: &&Exercise| self.filter.is_none_or(|m| exercise.works(m));
        let mut rows: Vec<PickRow<'a>> = Vec::new();
        if self.query.is_empty() {
            rows.extend(
                tracker
                    .recents()
                    .filter(works)
                    .map(|exercise| PickRow::Exercise {
                        exercise,
                        section: Section::Recent,
                    }),
            );
        }
        let section = if self.query.is_empty() {
            Section::All
        } else {
            Section::Matches
        };
        rows.extend(
            tracker
                .catalog
                .search(self.query.value(), self.filter)
                .into_iter()
                .map(|exercise| PickRow::Exercise { exercise, section }),
        );
        rows.push(PickRow::NewCustom);
        rows
    }

    pub fn handle_key(&mut self, key: KeyEvent, tracker: &mut Tracker) -> SheetEvent {
        match &mut self.step {
            Step::Pick => self.pick_key(key, tracker),
            Step::Sets(sets) => match key.code {
                KeyCode::Esc | KeyCode::Backspace => {
                    self.step = Step::Pick;
                    SheetEvent::None
                }
                KeyCode::Enter => SheetEvent::Log {
                    exercise: sets.exercise.clone(),
                    count: sets.count,
                },
                KeyCode::Up | KeyCode::Right | KeyCode::Char('k' | 'l' | '+' | '=') => {
                    sets.step_by(1);
                    SheetEvent::None
                }
                KeyCode::Down | KeyCode::Left | KeyCode::Char('j' | 'h' | '-' | '_') => {
                    sets.step_by(-1);
                    SheetEvent::None
                }
                KeyCode::Char(c) if c.is_ascii_digit() => {
                    sets.type_digit(c.to_digit(10).unwrap_or(1));
                    SheetEvent::None
                }
                KeyCode::Char('n') => {
                    let note = tracker
                        .catalog
                        .get(&sets.exercise)
                        .map(Exercise::note)
                        .unwrap_or_default();
                    self.step = Step::Note(NoteEditor {
                        exercise: sets.exercise.clone(),
                        input: TextInput::new(NOTE_LIMIT).with_value(note),
                        count: sets.count,
                    });
                    SheetEvent::None
                }
                _ => SheetEvent::None,
            },
            Step::Note(editor) => match key.code {
                KeyCode::Esc => {
                    self.step = Step::Sets(SetCount::new(editor.exercise.clone(), editor.count));
                    SheetEvent::None
                }
                KeyCode::Enter => {
                    let exercise = editor.exercise.clone();
                    tracker.catalog.set_note(&exercise, editor.input.value());
                    let message = if editor.input.value().trim().is_empty() {
                        "Note cleared."
                    } else {
                        "Note saved."
                    };
                    self.step = Step::Sets(SetCount::new(exercise, editor.count));
                    SheetEvent::Saved(message.to_owned())
                }
                _ => {
                    editor.input.handle_key(key);
                    SheetEvent::None
                }
            },
            Step::Create(form) => match form.handle_key(key, tracker) {
                FormOutcome::Stay => SheetEvent::None,
                FormOutcome::Back => {
                    self.step = Step::Pick;
                    SheetEvent::None
                }
                FormOutcome::Created(name) => {
                    let message = format!("Created '{name}'.");
                    self.step = Step::Sets(SetCount::new(name, 1));
                    SheetEvent::Saved(message)
                }
            },
            Step::ConfirmDelete { exercise } => match key.code {
                KeyCode::Char('y' | 'Y') | KeyCode::Enter => {
                    let exercise = exercise.clone();
                    self.step = Step::Pick;
                    match tracker.catalog.remove_custom(&exercise) {
                        Ok(removed) => {
                            tracker.forget_recent(removed.name());
                            self.selected = self.selected.saturating_sub(1);
                            SheetEvent::Saved(format!(
                                "Deleted '{}'. Sets already logged are kept.",
                                removed.name()
                            ))
                        }
                        Err(err) => SheetEvent::Notice(err.to_string()),
                    }
                }
                KeyCode::Char('n' | 'N') | KeyCode::Esc => {
                    self.step = Step::Pick;
                    SheetEvent::None
                }
                _ => SheetEvent::None,
            },
        }
    }

    /// The wheel steps through the list or the set count; clicks pick rows
    /// and muscles. A second click on the selected row chooses it.
    pub fn handle_mouse(
        &mut self,
        target: MouseTarget,
        action: MouseAction,
        tracker: &mut Tracker,
    ) -> SheetEvent {
        let wheel = match action {
            MouseAction::ScrollUp => Some(KeyCode::Up),
            MouseAction::ScrollDown => Some(KeyCode::Down),
            MouseAction::Click => None,
        };
        match (&mut self.step, target, wheel) {
            // Anywhere on the sheet, the wheel works like the arrow keys.
            (Step::Pick | Step::Sets(_), target, Some(code)) if target != MouseTarget::Backdrop => {
                self.handle_key(KeyEvent::from(code), tracker)
            }
            (Step::Pick, MouseTarget::LogRow(row), None) if row == self.selected => {
                self.handle_key(KeyEvent::from(KeyCode::Enter), tracker)
            }
            (Step::Pick, MouseTarget::LogRow(row), None) => {
                self.selected = row;
                SheetEvent::None
            }
            (Step::Create(form), MouseTarget::CreateMuscle(muscle), None) => {
                form.focus = Field::Muscles;
                form.cursor = muscle;
                form.cycle_role();
                SheetEvent::None
            }
            _ => SheetEvent::None,
        }
    }

    /// Pasted text goes to whichever text field is active.
    pub fn paste(&mut self, text: &str) {
        match &mut self.step {
            Step::Pick => {
                self.query.insert_str(text);
                self.selected = 0;
            }
            Step::Note(editor) => editor.input.insert_str(text),
            Step::Create(form) if form.focus == Field::Name => form.name.insert_str(text),
            Step::Sets(_) | Step::Create(_) | Step::ConfirmDelete { .. } => {}
        }
    }

    fn pick_key(&mut self, key: KeyEvent, tracker: &Tracker) -> SheetEvent {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let rows = self.rows(tracker);
        let last = rows.len().saturating_sub(1);
        match key.code {
            KeyCode::Esc if !self.query.is_empty() => {
                self.query.clear();
                self.selected = 0;
            }
            KeyCode::Esc => return SheetEvent::Close,
            KeyCode::Up => self.selected = self.selected.saturating_sub(1),
            KeyCode::Char('p') if ctrl => self.selected = self.selected.saturating_sub(1),
            KeyCode::Down => self.selected = (self.selected + 1).min(last),
            KeyCode::Char('n') if ctrl => self.selected = (self.selected + 1).min(last),
            KeyCode::PageUp => self.selected = self.selected.saturating_sub(PAGE),
            KeyCode::PageDown => self.selected = (self.selected + PAGE).min(last),
            KeyCode::Tab => self.cycle_filter(1),
            KeyCode::BackTab => self.cycle_filter(-1),
            KeyCode::Char('d') if ctrl => {
                return match rows.get(self.selected) {
                    Some(PickRow::Exercise { exercise, .. })
                        if exercise.origin() == Origin::Custom =>
                    {
                        self.step = Step::ConfirmDelete {
                            exercise: exercise.name().to_owned(),
                        };
                        SheetEvent::None
                    }
                    Some(PickRow::Exercise { .. }) => {
                        SheetEvent::Notice("Built-in exercises cannot be deleted.".to_owned())
                    }
                    Some(PickRow::NewCustom) | None => SheetEvent::None,
                };
            }
            KeyCode::Enter => match rows.get(self.selected) {
                Some(PickRow::Exercise { exercise, .. }) => {
                    self.step = Step::Sets(SetCount::new(exercise.name().to_owned(), 1));
                }
                Some(PickRow::NewCustom) | None => {
                    let mut form = CreateForm::new(self.query.value());
                    if let Some(muscle) = self.filter {
                        form.cursor = muscle;
                        form.roles[muscle.index()] = Some(Role::Primary);
                    }
                    self.step = Step::Create(form);
                }
            },
            _ => {
                let before = self.query.value().to_owned();
                self.query.handle_key(key);
                if self.query.value() != before {
                    self.selected = 0;
                }
            }
        }
        SheetEvent::None
    }

    fn cycle_filter(&mut self, step: isize) {
        // Positions: 0 is "all muscles", then each muscle in order.
        let positions = Muscle::ALL.len() + 1;
        let current = self.filter.map_or(0, |m| m.index() + 1);
        let next = current.wrapping_add_signed(step).wrapping_add(positions) % positions;
        self.filter = next.checked_sub(1).map(|index| Muscle::ALL[index]);
        self.selected = 0;
    }
}
