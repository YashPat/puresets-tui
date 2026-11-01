# PureSets TUI Agent Context

Terminal version of PureSets, a minimalist gym tracker built around weekly hard sets per muscle group. The iOS app lives in `../Pure-Sets`; the marketing site in `../PureSetsWebsite`. Read `../Pure-Sets/AGENTS.md` for the product thesis before designing features.

## Product rules (carried over from the app)

- Track weekly set volume by muscle group; one exercise can credit multiple muscles.
- No weight, reps, rest timers or RPE. Logging must stay near-frictionless.
- Treat the research as inspiration, never as a prescription engine.

## Stack

- Rust (edition 2024), [ratatui](https://ratatui.rs) + crossterm, color-eyre for errors.
- Lints are in `Cargo.toml` (`[lints]`) and `clippy.toml`; CI-strict: `cargo clippy --all-targets -- -D warnings`.

## Skills

Source of truth is `.rulesync/skills/`; `rulesync generate` writes `.claude/skills/`.

- `rust-best-practices`: load whenever writing or changing Rust.
- `rust-review`: review a diff or crate.
- `tui-design`: use for any screen, layout, keybinding or theme decision. Follow `references/` and `frameworks/ratatui-rust.md`.

## Commands

- `cargo run`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt`
