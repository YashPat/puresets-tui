//! `PureSets` in the terminal: track weekly hard sets per muscle group.

mod app;
mod domain;
mod storage;
mod ui;

use std::{
    io::{self, IsTerminal},
    path::{Path, PathBuf},
    process::ExitCode,
    time::{Duration, Instant},
};

use color_eyre::{
    Result,
    eyre::{WrapErr, bail, eyre},
};
use ratatui::{
    DefaultTerminal,
    crossterm::{
        event::{self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyEventKind},
        execute,
    },
};

use app::{App, ToastKind};
use domain::{Catalog, Muscle, Week};
use storage::{DATA_ENV, Loaded, Storage};
use ui::Theme;

/// How long to wait for input before refreshing the clock and toasts.
const POLL_INTERVAL: Duration = Duration::from_millis(250);

const USAGE: &str = "\
PureSets: track weekly hard sets per muscle group.

Usage: puresets-tui [OPTIONS]

Options:
  --data <PATH>   Use this data file (default: $PURESETS_DATA, or the
                  platform data directory, e.g. ~/Library/Application Support/puresets/data.json)
  --summary       Print this week's sets as plain text and exit
  -h, --help      Print this help
  -V, --version   Print the version

Press ? inside the app for keyboard shortcuts.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Interactive,
    Summary,
    Help,
    Version,
}

#[derive(Debug, PartialEq, Eq)]
struct Args {
    mode: Mode,
    data: Option<PathBuf>,
}

fn parse_args(mut args: impl Iterator<Item = String>) -> Result<Args> {
    let mut parsed = Args {
        mode: Mode::Interactive,
        data: None,
    };
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => parsed.mode = Mode::Help,
            "-V" | "--version" => parsed.mode = Mode::Version,
            "--summary" => parsed.mode = Mode::Summary,
            "--data" => {
                let path = args
                    .next()
                    .ok_or_else(|| eyre!("--data needs a file path"))?;
                parsed.data = Some(PathBuf::from(path));
            }
            other => match other.strip_prefix("--data=") {
                Some(path) => parsed.data = Some(PathBuf::from(path)),
                None => bail!("unknown argument '{other}'\n\n{USAGE}"),
            },
        }
    }
    Ok(parsed)
}

fn main() -> ExitCode {
    // color-eyre's panic hook gives readable crash reports; errors we expect
    // (bad arguments, an unreadable data file) print as one plain line.
    if let Err(err) = color_eyre::install().and_then(|()| run()) {
        print_err(&format!("error: {err:#}"));
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn run() -> Result<()> {
    let args = parse_args(std::env::args().skip(1))?;
    match args.mode {
        Mode::Help => print_out(USAGE),
        Mode::Version => print_out(&format!("puresets-tui {}", env!("CARGO_PKG_VERSION"))),
        Mode::Summary => {
            let storage = storage_for(args.data)?;
            let loaded = storage.load(Catalog::built_in()?)?;
            print_out(&summary(
                &loaded,
                Week::containing(jiff::Zoned::now().date()),
            ));
        }
        Mode::Interactive => {
            if !io::stdout().is_terminal() {
                bail!(
                    "PureSets needs an interactive terminal. Use --summary for plain-text output."
                );
            }
            let storage = storage_for(args.data)?;
            // Load before entering fullscreen so errors print normally.
            let loaded = storage.load(Catalog::built_in()?)?;
            run_interactive(&storage, loaded)?;
        }
    }
    Ok(())
}

#[expect(
    clippy::print_stdout,
    reason = "the non-interactive modes write to stdout"
)]
fn print_out(text: &str) {
    println!("{text}");
}

#[expect(clippy::print_stderr, reason = "errors are reported on stderr")]
fn print_err(text: &str) {
    eprintln!("{text}");
}

fn storage_for(data: Option<PathBuf>) -> Result<Storage> {
    match data {
        Some(path) => Ok(Storage::new(path)),
        None => Storage::default_location()
            .wrap_err_with(|| format!("finding a place for the data file (set {DATA_ENV})")),
    }
}

fn run_interactive(storage: &Storage, loaded: Loaded) -> Result<()> {
    let Loaded { tracker, warnings } = loaded;
    let today = jiff::Zoned::now().date();
    let mut app = App::new(tracker, today, Instant::now(), display_path(storage.path()));
    if !warnings.is_empty() {
        app.notify(
            ToastKind::Error,
            format!("Skipped part of the data file: {}", warnings.join("; ")),
        );
    }

    // `init` also installs a panic hook that restores the terminal.
    let mut terminal = ratatui::init();
    // Bracketed paste is an enhancement: without it, pasted text arrives as keys.
    let _ = execute!(io::stdout(), EnableBracketedPaste);
    let result = event_loop(&mut terminal, &mut app, storage);
    let _ = execute!(io::stdout(), DisableBracketedPaste);
    ratatui::restore();
    result
}

fn event_loop(terminal: &mut DefaultTerminal, app: &mut App, storage: &Storage) -> Result<()> {
    let theme = Theme::from_env();
    let mut unsaved = false;
    while !app.should_quit() {
        terminal.draw(|frame| ui::draw(frame, app, theme))?;
        if event::poll(POLL_INTERVAL)? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => app.handle_key(key),
                Event::Paste(text) => app.paste(&text),
                _ => {}
            }
        }
        app.tick(Instant::now(), jiff::Zoned::now().date());

        // A failed save is retried on every pass until it works.
        if app.take_dirty() || unsaved {
            match storage.save(&app.tracker) {
                Ok(()) => unsaved = false,
                Err(err) => {
                    if !unsaved {
                        app.notify(ToastKind::Error, format!("Could not save: {err:#}"));
                    }
                    unsaved = true;
                }
            }
        }
    }
    if unsaved {
        storage
            .save(&app.tracker)
            .wrap_err("your last changes could not be saved")?;
    }
    Ok(())
}

/// The data path for display, with the home directory shortened to `~`.
fn display_path(path: &Path) -> String {
    match dirs::home_dir().and_then(|home| path.strip_prefix(home).ok().map(Path::to_path_buf)) {
        Some(relative) => format!("~/{}", relative.display()),
        None => path.display().to_string(),
    }
}

/// This week's sets as a plain-text table, for scripts and pipes.
fn summary(loaded: &Loaded, week: Week) -> String {
    use std::fmt::Write as _;

    let tracker = &loaded.tracker;
    let target = tracker.target_or_default();
    let mut out = format!(
        "PureSets · Week {} ({}) · target {target} sets/muscle\n\n",
        week.number(),
        week.range_label()
    );
    let mut category = None;
    for muscle in Muscle::ALL {
        if category != Some(muscle.category()) {
            if category.is_some() {
                out.push('\n');
            }
            category = Some(muscle.category());
            let _ = writeln!(out, "{}", muscle.category().name().to_uppercase());
        }
        let sets = tracker.log.sets(week, muscle);
        let mark = if sets >= target.sets() { "✓" } else { " " };
        let _ = writeln!(out, "  {:<14}{sets:>5} / {target}  {mark}", muscle.name());
    }
    let _ = write!(out, "\nTotal {} sets", tracker.log.week_total(week));
    for warning in &loaded.warnings {
        let _ = write!(out, "\nwarning: {warning}");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Result<Args> {
        parse_args(list.iter().map(|&arg| arg.to_owned()))
    }

    #[test]
    fn parses_arguments() {
        assert_eq!(args(&[]).unwrap().mode, Mode::Interactive);
        assert_eq!(args(&["--summary"]).unwrap().mode, Mode::Summary);
        assert_eq!(args(&["-V"]).unwrap().mode, Mode::Version);
        let parsed = args(&["--data", "x.json", "-h"]).unwrap();
        assert_eq!(parsed.mode, Mode::Help);
        assert_eq!(parsed.data, Some(PathBuf::from("x.json")));
        assert_eq!(
            args(&["--data=y.json"]).unwrap().data,
            Some(PathBuf::from("y.json"))
        );
        assert!(args(&["--data"]).is_err());
        assert!(args(&["--bogus"]).is_err());
    }

    #[test]
    fn summary_lists_every_muscle() {
        let mut tracker = domain::Tracker::new(Catalog::built_in().unwrap());
        let week = Week::containing(jiff::civil::date(2026, 11, 1));
        tracker.log_sets(week, "Bench Press", 3).unwrap();
        let loaded = Loaded {
            tracker,
            warnings: Vec::new(),
        };
        let text = summary(&loaded, week);
        assert!(
            text.starts_with("PureSets · Week 44 (Oct 26 – Nov 1)"),
            "{text}"
        );
        assert!(text.contains("  Chest             3 / 8"), "{text}");
        for muscle in Muscle::ALL {
            assert!(text.contains(muscle.name()), "{} missing", muscle.name());
        }
    }
}
