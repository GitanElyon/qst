# qst API

This document describes the base qst launcher internals and how script packs integrate with it.

The script catalog and community scripts are documented in:

- https://github.com/gitanelyon/awesome-qst

## Core modules

- `src/main.rs`
  - Terminal lifecycle, event loop, key handling.
- `src/app.rs`
  - App state machine, filtering, file explorer, launch argument handling, command spawning.
- `src/ui.rs`
  - Rendering and list presentation.
- `src/config.rs`
  - `config.toml` loading and defaults.
- `src/history.rs`
  - App usage/favorites history persistence.

## Configuration surface

qst reads:

- `~/.config/qst/config.toml` (UI + launcher behavior)
- `~/.config/qst/alias.toml` (optional script/trigger mapping)

Important toggles in `features`:

- `enable-file-explorer` (default: `true`)
- `enable-launch-args` (default: `true`)
- `enable-auto-complete` (default: `true`)
- `dirs-first` (default: `true`)

## File explorer behavior

Implemented in `App::update_filter` + `App::list_completions`:

- Path queries beginning with `/`, `~/`, `./`, or `../` enter file-selection mode.
- Tab completion in file-selection mode inserts selected paths.
- If selected path is executable, qst runs it directly; otherwise it opens via `xdg-open`.

## Script pack integration

- Scripts are expected under `~/.config/qst/scripts/`.
- Scripts may be executable files (run directly) or known extension files run via interpreter (`.sh`, `.bash`, `.zsh`, `.fish`, `.py`, `.pl`, `.rb`, `.js`, `.lua`).
- Script and app aliases are loaded from `~/.config/qst/alias.toml`.

For script implementation details and curated scripts, use the `awesome-qst` repo.
