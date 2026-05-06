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
actions, and a clipboard stub.

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
- clipboard-history placeholder state.

The binary initializes this state and exits. It does not register hotkeys, watch
config files, expose IPC, or capture clipboard changes.

### freepalette-ui

The UI crate contains a minimal egui palette. It can search, move selection, and
execute selected non-shell actions through `freepalette-daemon`. Shell actions
are shown but blocked because there is no confirmation UI yet.

There is no global hotkey, tray integration, IPC daemon connection, or polished
desktop shell.

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

Shortcuts and ClickOnce entries are launched through `explorer.exe`. Direct
`.exe` entries are launched by path. If indexing is unsupported, unavailable, or
empty, the provider records that state and uses a labeled Notepad fallback only
when there are no configured apps.

## Current Limits

- App indexing is Windows-first.
- Clipboard history is a stub.
- The daemon crate is not an IPC process.
- Global hotkeys are not implemented.
- External plugin execution is not implemented.
- The UI is usable for smoke testing but is not a finished launcher.
