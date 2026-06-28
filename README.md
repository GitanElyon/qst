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

Note: qst bundles a helper script, `loader.sh`, and will install it into `~/.config/qst/scripts/` on first run so users can browse and install community plugins without manually copying files.

Scripts can be executable files (any language) or extension-based scripts run through supported interpreters (`.sh`, `.bash`, `.zsh`, `.fish`, `.py`, `.pl`, `.rb`, `.js`, `.lua`).

The plugin ecosystem is cataloged in `awesome-qst`:
- https://github.com/gitanelyon/awesome-qst

## Install

Install via Nix (recommended):
```bash
nix profile install "github:GitanElyon/qst"
```

Or via the AUR (Arch Linux):
```bash
yay -S qst
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
qst --config <path>       # points Qst to a new different config file
qst --prefill <string>    # seeds the initial search text
qst --shy                 # opens launcher with text hidden untill query is entered
qst --no-fuzzy            # disables fuzzy finding
qst --clear-history       # clears Qst's launch history
qst --clear-favorites     # clears favorite list
qst --program <program>   # launches the first result of the query
qst --script <script>     # opens the script on startup
qst --list-programs       # lists available programs
qst --list-scripts        # lets available scripts
```

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
