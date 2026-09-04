# Changelog

All notable changes to Qst are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.13.0] - 2026-09-04

### Added

- Scripts are loaded from directories recursively.
- Scripts execute asynchronously in the background; identical payloads are skipped.
- Qst maintains its own script catalog, refreshed automatically on open.
- New script management arguments:
  - `--install <script>`
  - `--remove <script>`
  - `--update <script>`
  - `--refresh-catalog`

### Changed

- Script names and aliases now require a delimiting space before a payload (`v! clip`, not `v!clip`).

### Removed

- Automatic download of `loader.sh`.

## [0.12.0] - 2026-08-26

## [0.12.0] - 2026-08-26

### Added

- CI workflow running `fmt`, `clippy`, `test`, and release builds on every push and pull request.
- `CONTRIBUTING.md` with contribution guidelines.

### Fixed

- All remaining clippy warnings (`-D warnings` now passes cleanly).
- Spelling errors in the README and CLI documentation.

### Changed

- Standardized terminology: the docs and bundled scripts now consistently refer to `scripts` instead of mixing `script`, `plugin`, and `extension`.
- Moved the script API reference into the main repository (`API.md`), where it previously lived in `awesome-qst`.
- Refactored the README for clarity and structure; added a screenshot.
- Converted the qst logo from ASCII art to an SVG path.
- The development shell (`flake.nix`) now includes `clippy` and `rustfmt`.

## [0.11.0] - 2026-07-18

### Added

- Logging to `~/.local/state/qst/qst.log` with four levels: `debug`, `info`, `warn`, `error`.
- Debug overlay showing FPS, frame time, entry count, and event count.
- New arguments:
  - `--debug-overlay` to start with the debug overlay visible.
  - `--log-level <level>` to set the log level.
- Log retention options (`log_retention_days`) with automatic pruning of archived sessions.
- `qst! log` directive for script logging.
- Panic hook and flight recorder ring buffer to flush events on crash.

### Changed

- Session logs are archived to `~/.local/state/qst/sessions/<timestamp>.log` on startup.

## [0.10.0] - 2026-06-16

### Added

- New arguments:
  - `--program <name>` / `-p` to launch a program directly.
  - `--script <name>` / `-s` to open a script by default.
  - `--config <path>` to use a custom config file.
  - `--prefill <string>` to seed the initial search text.
  - `--shy` to hide entries until typing starts.
  - `--no-fuzzy` to disable fuzzy matching.
  - `--clear-history` and `--clear-favorites`.
  - `--list-programs` and `--list-scripts`.
  - `--version` / `-v`.
- `loader.sh` is now bundled with qst and installed into `~/.config/qst/scripts/` on first run.
- AUR installation support.

### Changed

- Replaced `reqwest` with a lightweight `rustls`-based fetch for the bundled loader script.

### Fixed

- `RefreshResults` not re-running the script.

## [0.9.0] - 2026-04-24

### Added

- Script metadata (name, author, version, description).
- `center` metadata token.
- File system management API calls.
- Directive chaining.
- Config validation warnings and improved key binding parsing.
- Script guardrails and increased test coverage.

### Changed

- Script API signal renamed from `f!` to `qst!` to match the rebrand.
- Updated `urgent` and `active` metadata token functionality.
- Reworked the file browser to be more intuitive.

### Removed

- Compound directives, replaced with standalone ones.
- Redundant default handling.

## [0.8.2] - 2026-04-01

### Changed

- Project renamed from `flare` to `qst`.

### Removed

- Categories from `Cargo.toml`.

## [0.8.1] - 2026-03-31

### Added

- New API calls: `CopyToClipboard`, `PopLastToken`, `SetStatusMessage`, `ClearStatusMessage`, `PrependToQuery`, `ReplaceLastToken`, `PopLastChar`, `RefreshResults`.
- API metadata options: `display`, `meta`, `nonselectable`, `permanent`, `active`, `urgent`.

### Changed

- README now includes the app logo.

## [0.8.0] - 2026-03-23

### Added

- Script system: app features turned into modular scripts.
- Alias system for scripts and apps.
- Line-oriented API protocol for scripts to talk to qst.

## [0.7.0] - 2026-02-18

### Changed

- Configuration overhaul: easier configuration, more options.
- Enhanced input editing functionality.

### Added

- `--gen-config` to generate a default config file.
- ASCII title text support.

## [0.6.0] - 2026-01-31

### Added

- Calculator functionality (activated with the `=` prefix), supporting advanced calculations like limits.
- Math history.

## [0.5.0] - 2026-01-28

### Added

- Nerd Font symbol selection (activated with the `.` prefix).
- Enhanced fuzzy matching and result sorting.

## [0.4.0] - 2025-12-04

### Added

- Sudo launch options.
- Results box title customization.
- Nix flake and flake.lock.

## [0.3.0] - 2025-12-03

### Added

- App favorite toggle (`Alt+f`).
- Quick jump to top/bottom of list (`Alt+Up`/`Alt+Down`).
- Sort by recently used option.

## [0.2.1] - 2025-11-29

### Added

- `show-duplicates` config option.

## [0.2.0] - 2025-11-29

### Added

- Directory browsing (file explorer mode).
- File filtering for path queries.
- Launch argument handling.
- Fuzzy search.
- Tab auto-complete and improved path handling in file selection.

### Fixed

- Bug where launch arguments would not work.

## [0.1.0] - 2025-11-27

### Added

- Initial release: `.desktop` app scanning, basic configuration, and command execution.