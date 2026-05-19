# qst Docs

Here you will find documentation for qst features, configuration, and plugin integration.

Plugin packs (scripts, aliases, community catalog) are documented in:

- https://github.com/gitanelyon/awesome-qst

## Config files

qst reads launcher settings from:

- `~/.config/qst/config.toml`
  - UI + launcher behavior.

Script integration uses:

- `~/.config/qst/scripts/`
  - executable `*.sh` scripts discovered dynamically.
- `~/.config/qst/alias.toml`
  - optional trigger aliases for script names.

`config.toml` is created automatically on first run.

## Command-line options

qst also supports launch-time flags:

- `--config <path>`
  - Use a config file from a custom path.
- `--prefill <string>`
  - Launch qst with an initial search string.
- `--shy`
  - Hide entries until you start typing.
- `--no-fuzzy`
  - Disable fuzzy finding in the launcher.
- `--clear-history`
  - Clear qst's app history.
- `--clear-favorites`
  - Clear qst's favorite apps.
- `-p, --program <name>`
  - Launch a program directly using fuzzy matching and exit without opening the UI.
- `-s, --script <script>`
  - Start qst with that script opened by default.
- `--list-programs`
  - Print all launchable desktop programs.
- `--list-scripts`
  - Print all scripts and their metadata.
- `--gen-config`
  - Generate `~/.config/qst/config.toml` or the file given by `--config`.
- `-h, --help`
  - Print the CLI help text.

`--list-scripts` reads each script's metadata header from the script source file, matching the `qst! meta ...` convention used by the plugin docs.

## Important defaults

From `[features]` in `config.toml`:

- `enable-file-explorer = true`
- `enable-launch-args = true`
- `enable-auto-complete = true`
- `dirs-first = true`
- `show-duplicates = false`
- `recent-first = true`

## File explorer behavior

With file explorer enabled (default), typing a path query enters file-selection mode:

- Absolute path: `/...`
- Home path: `~/...`
- Relative path: `./...` or `../...`

Behavior:

- `Tab` autocompletes selected path.
- `Enter` on directories keeps browsing.
- `Enter` on files opens via `xdg-open`.
- Executable files can be executed directly.

## Keybindings

- `Up/Down`: move selection
- `Left/Right`: move input cursor
- `Tab`: autocomplete
- `Alt+f`: favorite/unfavorite app
- `Alt+Up`: jump to first item
- `Alt+Down`: jump to last item
- `Enter`: launch/open selected item
- `Esc`: quit

## Plugin integration notes

- qst is host/runtime.
- Script plugins live in `~/.config/qst/scripts/`.
- Protocol, directives, and setup guidance are in `https://github.com/gitanelyon/awesome-qst`.

## XDG app scan paths

qst discovers `.desktop` entries from standard XDG locations including:

- `/usr/share/applications`
- `/usr/local/share/applications`
- `~/.local/share/applications`