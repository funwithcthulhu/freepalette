# Architecture

freepalette is a Rust workspace with a small core and thin outer crates. The
current code is still early. This document describes what exists, not what the
project might become later.

## Crates

### freepalette-core

Owns the launcher domain code:

- TOML config loading;
- provider registration;
- built-in providers;
- fuzzy matching;
- ranking;
- Windows Start Menu app indexing;
- action dispatch through providers.

Built-in providers currently cover apps, calculator queries, shell command
actions, and clipboard history results backed by explicit in-memory daemon
state.

### freepalette-cli

Owns command parsing and terminal output. It can:

- search providers;
- print provider IDs;
- inspect app indexing state;
- print the default config path;
- run the top result after explicit user request.

Shell actions are blocked unless the user passes `--allow-shell`.

### freepalette-daemon

Despite the crate name, this is not a long-running daemon yet. It currently
holds shared local state used by the CLI and UI:

- loaded config source;
- provider registry;
- app index report;
- search;
- app index refresh;
- action execution policy;
- in-memory clipboard-history state;
- global-hotkey config state;
- Windows foreground hotkey diagnostics.

The default binary command initializes this state and exits. `freepalette-daemon
run` can register the configured hotkey on Windows and wait in the foreground.
That diagnostic path logs configured hotkey presses. The crate does not expose
IPC, watch config files, capture clipboard changes, or open/focus the UI from a
separate daemon process yet.

### freepalette-ui

The UI crate contains a minimal Tauri palette shell. The frontend is static
HTML, CSS, and JavaScript. It invokes Rust commands in the same process to:

- search through `PaletteState`;
- move the selected result;
- execute selected non-shell actions;
- reload config through daemon state;
- reset visible palette state.

Shell actions are shown during search and require an explicit confirmation
prompt before the UI passes them to the daemon with shell execution allowed.
On Windows, the Tauri binary also owns the configured global hotkey, a tray
icon, and launch-at-sign-in tray actions. If the tray or hotkey lifecycle is
active, Escape and window close hide the palette instead of exiting the process.
If no background lifecycle exists, Escape closes the window.

There is no IPC daemon connection or finished desktop shell.

### freepalette-plugin-api

Defines data types shared by built-in providers and future plugin protocol
work:

- `SearchQuery`
- `SearchContext`
- `SearchResult`
- `Action`
- `ActionOutcome`
- `Provider`

The Rust trait is for in-repo providers. External plugin execution is not
implemented.

## Provider Flow

1. Load `Config`.
2. Build `DaemonState`, which registers enabled providers.
3. Pass a query string into `ProviderRegistry::search`.
4. Each provider returns zero or more `SearchResult` values.
5. Core ranking filters and sorts the results.
6. CLI or UI displays the ranked results.
7. Execution happens only after explicit user action.
8. The selected provider receives the selected action.

Search and execution are separate on purpose. Shell commands, app launches,
clipboard writes, and future plugin actions must not run because a query merely
matched.

## Config Flow

`Config` lives in `freepalette-core`. It can load from an explicit TOML path or
from the platform default path if a file exists there. Missing default config is
not an error; freepalette uses `Config::default()`.

The daemon state owns the loaded config and rebuilds provider state from it.
Tests use explicit temporary config files so they do not depend on a developer's
local machine.

Clipboard capture and global hotkeys are disabled by default. The hotkey config
is validated through daemon state and can be used by the foreground daemon
diagnostic loop. On Windows, the Tauri UI also reads the same hotkey config at
startup and registers the binding when it is enabled. Clipboard config is parsed
now so future capture work has a tested place to attach platform behavior.

## Ranking

Ranking is simple:

- fuzzy score over title, subtitle, and keywords;
- provider score hints for command-style results such as calculator and shell;
- exact-title and prefix-title bonuses;
- small result-kind bias for current MVP ergonomics;
- title and ID ordering as the final tie-breakers.

The constants are approximate. There is no personalization, telemetry, usage
history, account state, or cloud ranking.

## Windows App Indexing

The app provider scans these Start Menu roots on Windows:

- `%APPDATA%\Microsoft\Windows\Start Menu\Programs`
- `%ProgramData%\Microsoft\Windows\Start Menu\Programs`

It recursively indexes `.lnk`, `.exe`, and `.appref-ms` files. Configured apps
are loaded first and win over discovered apps with the same display name. User
Start Menu entries are checked before system entries.

Shortcut-like entries open through the Windows shell. Direct `.exe` entries are
launched by path. If indexing is unsupported, unavailable, or empty, the
provider records that state and uses a labeled Notepad fallback only when there
are no configured apps.

## Current Limits

- App indexing is Windows-first.
- Clipboard history is in-memory only and must be populated explicitly by
  daemon code. System clipboard capture and persistence must follow
  [Clipboard Security Model](CLIPBOARD_SECURITY.md).
- The daemon crate is not an IPC process.
- Global hotkey registration in the Tauri UI is Windows-only and disabled by
  default. The foreground daemon diagnostic path remains separate. See
  [Hotkeys](HOTKEYS.md).
- Tray integration and launch-at-sign-in tray actions are Windows-only.
- Installer packaging is not implemented.
- The UI has a basic shell-confirmation prompt, not a polished shell action
  review surface.
- External plugin execution is not implemented.
- The UI is usable for smoke testing but is not a finished launcher.
