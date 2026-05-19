<p align="center">
  <img src="assets/qst.svg" alt="qst Logo" width="577">
</p>


Qst, pronounced "quest", is a TUI Linux application launcher built with Rust + Ratatui.

## Highlights

- Fast `.desktop` app scanning and fuzzy search.
- Usage/favorites-based ordering.
- Launch arguments support.
- File explorer mode enabled by default.
- Keyboard-first navigation and customization.
- Extensible plugin system with script-based plugins.

## Plugin model

qst is the host runtime. Plugins are script-based and live in `~/.config/qst/scripts/`. Plugins can define custom triggers, query handling, and output formatting via a simple line-oriented protocol. 

Scripts can be executable files (any language) or extension-based scripts run through supported interpreters (`.sh`, `.bash`, `.zsh`, `.fish`, `.py`, `.pl`, `.rb`, `.js`, `.lua`).

The plugin ecosystem is cataloged in `awesome-qst`:
- https://github.com/gitanelyon/awesome-qst

## Install

Install via Nix (recommended):
```bash
nix profile install "github:GitanElyon/qst"
```

Or via Cargo:
```bash
cargo install --locked qst
```

Or build from source:
```bash
git clone https://github.com/GitanElyon/qst.git
cd qst
cargo install --locked --path .
```

## Usage

Either run qst from the terminal:

```bash
qst
```

You can also use launch-time flags:

```bash
qst --config ~/.config/qst/custom.toml
qst --prefill "runner"
qst --shy
qst --no-fuzzy
qst --clear-history
qst --clear-favorites
qst --program firefox
qst --script runner
qst --list-programs
qst --list-scripts
```

`--config` points qst at a different config file. `--prefill` seeds the initial search text. `--shy` starts with the launcher list hidden until you type. `--no-fuzzy` disables fuzzy finding. `--clear-history` clears qst's app history. `--clear-favorites` clears favorite apps. `-p, --program` launches a desktop program directly using qst's fuzzy matching. `-s, --script` opens that script by default when qst starts. The list flags print the available programs or scripts, including script metadata parsed from the script source header.

Or bind to a global hotkey (e.g. `Super+Space`) using your desktop environment's keyboard settings.

Example for hyperland users to mimic `rofi`:
```
bind = $mod, space, exec, [float; size 350 400] $terminal -e qst
```


## Keybindings

- `Up`/`Down`: move selection
- `Left`/`Right`: move cursor in input
- `Tab`: autocomplete path
- `Enter`: launch/open selected item
- `Esc`: quit
- `Alt+f`: toggle favorite

## Config files

- `~/.config/qst/config.toml`
- `~/.config/qst/alias.toml` (optional script and app aliases)

See [DOCS.md](DOCS.md) for full configuration details.
