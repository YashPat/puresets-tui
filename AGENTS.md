# PureSets TUI Agent Context

Terminal version of PureSets, a minimalist gym tracker built around weekly hard sets per muscle group. The iOS app lives in `../Pure-Sets`; the marketing site in `../PureSetsWebsite`. Read `../Pure-Sets/AGENTS.md` for the product thesis before designing features.

## Product rules (carried over from the app)

- Track weekly set volume by muscle group; one exercise can credit multiple muscles.
- No weight, reps, rest timers or RPE. Logging must stay near-frictionless.
- Treat the research as inspiration, never as a prescription engine.

## Stack

- Rust (edition 2024), [ratatui](https://ratatui.rs) + crossterm, color-eyre for errors.
- Lints are in `Cargo.toml` (`[lints]`) and `clippy.toml`; CI-strict: `cargo clippy --all-targets -- -D warnings`.

## Installed vs development copy

The user runs a release build installed with `cargo install --path .` (`~/.cargo/bin/puresets-tui`) and logs real workouts with it. Development must never touch that copy or its data.

- Data: release builds default to `<data dir>/puresets/data.json`; debug builds (`cargo run`, `cargo test`) default to `<data dir>/puresets-dev/data.json` (see `Storage::default_location`). Keep that split intact.
- Never point `--data` or `PURESETS_DATA` at the live `puresets/data.json` when running or testing. Use a scratch file for experiments.
- Never run `cargo install` unless the user asks to update the installed app. Updating it keeps its data, since the data file is outside the binary.
- Data format changes must stay backward compatible with the live file (old files load, unknown fields are tolerated), because the installed app and the dev build both read real-shaped data.

## Skills

Source of truth is `.rulesync/skills/`; `rulesync generate` writes `.claude/skills/`.

- `rust-best-practices`: load whenever writing or changing Rust.
- `rust-review`: review a diff or crate.
- `tui-design`: use for any screen, layout, keybinding or theme decision. Follow `references/` and `frameworks/ratatui-rust.md`.

## Commands

- `cargo run`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt`
