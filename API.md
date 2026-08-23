# Qst API

This document describes the script API contract for qst.

The script catalog and community scripts are documented in:

- https://github.com/gitanelyon/awesome-qst

# Script API

This is the contract for scripts placed in `~/.config/qst/scripts/`. qst is the host runtime; scripts are standalone programs that communicate with it through a line-oriented protocol on standard output.

Supported script styles:

- Executable scripts or binaries, run directly.
- Non-executable scripts with supported extensions, dispatched through an interpreter:
  - `.sh`, `.bash`, `.zsh`, `.fish`
  - `.py`
  - `.pl`
  - `.rb`
  - `.js`
  - `.lua`

## Invocation model

- Scripts are invoked with the current query payload as command-line arguments.
- qst parses standard output line by line.
- Empty lines are ignored.

## Item output

- `Title|Value` produces a list row with visible title `Title` and payload `Value`.
- `Title` is shorthand for a row where the payload defaults to the title.

## Host directives

Lines prefixed with `qst! ` are control directives:

- `qst! title <text>`
  - Sets the list title.
- `qst! message <text>`
  - Sets the status or message line under the input.
- `qst! clear_message`
  - Clears the status or message line.
- `qst! action <Action>`
  - Sets the default action for all list rows.
- `qst! item_action <Action>`
  - Sets the action for only the next emitted row.
- `qst! default_item_action <Action>`
  - Sets the action for all following rows unless overridden.
- `qst! item <title>|<value>|<Action>`
  - Emits an explicit row with an optional per-item action.
- `qst! clear`
  - Clears all accumulated rows.
- `qst! single <query>|<result>`
  - Returns a single-result response instead of list mode.
- `qst! write <file>|<WriteAction>|<value>`
  - Mutates `~/.config/qst/storage/<script-name>/<file>`.
- `qst! read <file>`
  - Reads the entire file and emits each stored line as a row.
- `qst! read <file>|<ReadAction>`
  - Reads the file with the specified action and emits the result as a single row.
- `qst! delete <file>`
  - Deletes the stored file.
- `qst! log <message>`
  - Writes a timestamped message to `~/.local/state/qst/<script-name>.log`.

## Script metadata

Scripts can declare a metadata header near the top of the file:

```bash
echo "qst! meta My Awesome script, 1.0.0, John Doe, This script does awesome things!"
```

The fields are, in order:

- `name`
- `version`
- `author`
- `description`

The header is parsed by qst as a first-class directive. After the header has been emitted, scripts can request one field at a time:

- `qst! meta name`
- `qst! meta version`
- `qst! meta author`
- `qst! meta description`

Helper scripts such as `help.sh` and `loader.sh` can read the same header to surface script details in their own UI, and they can also use the selector form when they need a single metadata field in a response.

Use the selector form when only one metadata field is needed.

## Row metadata

Row metadata is appended to a directive or row using `@meta:<key>=<value>` tokens.

Example:

```bash
echo "qst! item  Back|Back|PopLastToken @meta:permanent=true @meta:nonselectable=false"
```

Rules:

- Metadata tokens are appended at the end of the row or directive payload.
- Multiple metadata tokens may be chained in the same row.
- If a literal `/@meta` is used at the start of a row, qst treats it as normal text.

Supported row metadata keys:

- `display=<text>`
  - Overrides the visible label for the row.
- `fuzzy=<bool>`
  - Opts the row into host-side fuzzy filtering. When attached to a title or message directive, it enables fuzzy filtering for the entire script response.
- `meta=<terms>`
  - Adds hidden search terms. Separate multiple terms with commas.
- `nonselectable=<bool>`
  - Skips the row during selection movement.
- `permanent=<bool>`
  - Keeps the row available for UI flows that preserve permanent rows.
- `active=<bool>`
  - Renders the row with the active-state styling.
- `center=<bool>`
  - Centers the row within the available text area.
- `urgent=<bool>`
  - Moves the row to the top of the list and renders it in bold red without an inline prefix.

## Supported actions

Actions are evaluated left to right and can be combined with commas, for example `CopyToClipboard,ExitApp` or `Execute,RefreshResults`.

### Main Actions

- `CopyToClipboard`
  - Copies the value to the clipboard and keeps qst open.
- `SetStatusMessage`
  - Sets the status or message line to the row value.
- `ClearStatusMessage`
  - Clears the status or message line.
- `SetSearchQuery`
  - Replaces the current query with the row value.
- `AppendToQuery`
  - Appends the row value to the current query.
- `PrependToQuery`
  - Inserts the row value at the beginning of the current query.
- `ReplaceLastToken`
  - Replaces the trailing whitespace-delimited token with the row value.
- `PopLastToken`
  - Removes the trailing whitespace-delimited token from the current query.
- `PopLastChar`
  - Removes the last character from the current query.
- `ClearQuery`
  - Clears the entire query.
- `RefreshResults`
  - Re-runs filtering and script resolution without changing the query text.
- `Execute`
  - Executes the row value as a shell command and keeps qst open.
- `ExitApp`
  - Exits qst.
- `ResetPrompt`
  - Resets the search query back to its first token. For example, `todo n example` becomes `todo `. If it follows `Execute` in the same action list, qst waits for that command to finish first.
- `None`
  - No-op action; keeps qst open.

### Write Actions

- `pfront`
  - Pushes the value to the front of the file.
- `pback`
  - Pushes the value to the back of the file.
- `rmfront`
  - Removes a line from the front of the file.
- `rmback`
  - Removes a line from the back of the file.
- `purge`
  - Removes every stored line that exactly matches the provided value.

### Read Actions

- `fpeek`
  - Reads a line from the front of the file without removing it.
- `bpeek`
  - Reads a line from the back of the file without removing it.
- `all`
  - Reads the entire file content.

Storage files are created automatically when a write directive targets a missing file.

## Minimal example

```bash
#!/usr/bin/env bash
query="${1:-}"

echo "qst! title  Demo Script "

if [[ -z "$query" ]]; then
	echo "qst! action None"
	echo "  Type a command after the trigger|"
else
	echo "qst! action Execute,ExitApp"
	echo "  Run: $query|$query"
fi
```

For real-world patterns and authoring guidance, see the `awesome-qst` repository:

- https://github.com/gitanelyon/awesome-qst