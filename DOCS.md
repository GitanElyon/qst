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
  - Note: qst ships a built-in `loader.sh` helper and will seed it into this directory on first run. You can use `loader.sh` to browse and install scripts from the `awesome-qst` catalog.
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
- `--log-level <level>`
  - Set the log level: `debug`, `info`, `warn`, or `error` (default: `info`). Overrides `cargo.toml` log level.
-  `-v, --version`
  - Print the qst version.

`--list-scripts` reads each script's metadata header from the script source file, matching the `qst! meta ...` convention used by the plugin docs.

## Logging

Logs are written to `~/.local/state/qst/qst.log` in the format:

```
[2024-06-15 10:30:45.123] [INFO] [src/app.rs:127] message
```

### Log levels

| Level | Config value | Purpose |
|---|---|---|
| `DEBUG` | `debug` | All actions, every user movement, click, and render |
| `INFO` | `info` | When scripts are loaded or actions are taken |
| `WARN` | `warn` | Minor errors like a script having a parsing problem |
| `ERROR` | `error` | When a script won't load or the app crashes |

Default: `info`.

### Configuration

The log level can be set via the config file in the `[general]` section:

```toml
[general]
log_level = "debug"
```

Or via the `--log-level` CLI flag, which takes precedence over the config file:

```bash
qst --log-level debug
```

Valid values: `debug`, `info`, `warn`, `error`.

### Session rotation

Each session starts with a fresh `qst.log`. The previous session's log is automatically archived to `~/.local/state/qst/sessions/<YYYY-MM-DD_HH-MM-SS>.log` on startup.

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
