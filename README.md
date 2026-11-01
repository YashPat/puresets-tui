# puresets-tui

PureSets in the terminal. A minimalist gym tracker built around one number: **weekly hard sets per muscle group**. Log an exercise and every muscle it works gets credit: one set for primary movers, half a set for secondary movers. There are no weights, reps, timers or RPE.

It uses the same exercise list (`data/exercises.json`) and the same research curve (`data/hypertrophy-dose-response.csv`) as the iOS app. Both are compiled into the binary.

## Requirements

- Rust 1.87 or later, to build it
- An interactive terminal with UTF-8, at least **64×20** (80×24 or larger recommended)

## Installation

```bash
cargo install --path .
```

## Quick Start

```bash
puresets-tui
```

1. On first launch, choose a weekly target: **Moderate** (8 sets) or **Optimal** (12 sets).
2. Press `enter` on a muscle, or `a` to search every exercise.
3. Type to find the exercise, press `enter`, choose how many hard sets, then press `enter` again.
4. Home shows each muscle's progress toward the target this week.

For a plain-text table of this week's sets, for scripts or a shell prompt:

```bash
puresets-tui --summary
```

## Configuration

| Setting | How |
|---|---|
| Data file | `--data PATH`, or the `PURESETS_DATA` environment variable. The default is the platform data directory, e.g. `~/Library/Application Support/puresets/data.json` on macOS or `~/.local/share/puresets/data.json` on Linux. |
| Weekly target | Settings screen (4 to 20 sets per muscle) |
| Colour | Set `NO_COLOR=1`, or use `TERM=dumb`, for monochrome output |

Changes save automatically. The data file is plain JSON, written atomically, so you can back it up, sync it, or edit it while the app is closed. Custom exercises and notes are stored in it too.

## Keyboard Shortcuts

Press `?` in the app for the full list.

| Where | Key | Action |
|---|---|---|
| Everywhere | `1`–`4`, `tab` / `shift-tab` | Switch screen |
| | `?` | Help |
| | `q`, `ctrl-c` | Quit |
| Home | `enter` | Log sets for the selected muscle |
| | `a` or `/` | Log sets, searching all exercises |
| | `j`/`k`, `↑`/`↓` | Select a muscle |
| | `h`/`l`, `←`/`→` | Previous / next week |
| | `t` | Back to this week |
| | `+` / `-` | Add or remove one set on the selected muscle |
| | `]` / `[` | Add or remove half a set |
| | `u` | Undo |
| Log sheet | type | Search exercises |
| | `↑`/`↓`, `enter` | Choose an exercise, then set the count (`↑`/`↓` or type a number) |
| | `tab` | Filter by muscle |
| | `n` | Edit the exercise's note |
| | `ctrl-d` | Delete a custom exercise |
| | `esc` | Back / close |
| Insights | `h`/`l` | Change period (this week, 4 weeks, 52 weeks) |
| Why | `j`/`k`, `space` | Scroll |
| Settings | `h`/`l` | Change the target by one set |
| | `m` / `o` | Moderate / Optimal preset |

## Terminal Compatibility

| | |
|---|---|
| **Required** | A TTY, UTF-8, at least 64×20 |
| **Enhanced** | 16 colours or more; bracketed paste (pasting into search) |
| **Fallback** | `NO_COLOR` / `TERM=dumb` use bold, dim and reverse instead of colour. Without bracketed paste, pasted text arrives as ordinary key presses. |
| **Not used** | Mouse, truecolor, images |

It is built on crossterm, so it should work in any terminal crossterm supports: macOS and Linux terminals, Windows Terminal, and tmux. So far it has only been exercised through headless render tests and a scripted pseudo-terminal on macOS.

## Troubleshooting

- **"PureSets needs a bigger window"**: enlarge the window to at least 64×20.
- **"PureSets needs an interactive terminal"**: stdout is not a TTY (a pipe or CI). Use `--summary` instead.
- **"Skipped part of the data file"**: an entry in the data file was not understood (for example an unknown muscle or a malformed week) and was skipped. The rest of the file loaded normally. Fix the entry by hand if you need it back.
- **Garbled box drawing or bars**: the terminal or font is not UTF-8. Check `locale` and use a font with box-drawing characters.
- **The terminal is left in a strange state after a crash**: run `reset`. The app restores the terminal on normal exit, on errors and on panics, so please report it if this happens.

## Development

```bash
cargo run -- --data /tmp/puresets-dev.json   # a scratch data file
cargo fmt
cargo clippy --all-targets -- -D warnings
```

The code is in three layers:

- `src/domain`: muscles, exercises and set credits, ISO weeks, the weekly log, and insights. No terminal code.
- `src/app`: UI state and key handling (tabs, the log sheet, undo, toasts). No drawing.
- `src/ui`: rendering only, a pure function of app state and terminal size.

`src/storage.rs` reads and writes the data file. `src/main.rs` parses arguments and runs the event loop.

## Testing

```bash
cargo test
```

The tests cover the domain rules (credits, weeks, insights, catalog parsing), scripted key sequences through the app state, and headless render tests. The render tests draw real screens into ratatui's `TestBackend` at several sizes, including the minimum, in colour and monochrome.

### Notes on the shared data

`exercises.json` comes from the iOS app, with its duplicate "Chest Dips" entry removed. A few entries name muscles the app does not track. The catalog maps them to a tracked muscle (`MUSCLE_ALIASES` in `src/domain/catalog.rs`):

| In `exercises.json` | Credited to |
|---|---|
| `Shoulders` | Front Delts |
| `Core` | Abs |

So Overhead Press credits Front Delts and Triceps, plus half sets to Abs and Chest. Any other unknown muscle name is skipped.
