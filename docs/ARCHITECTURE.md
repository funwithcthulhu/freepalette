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
actions, and clipboard history results backed by local daemon state.

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
- clipboard-history state;
- local recency state for successful non-clipboard executions;
- local state persistence for clipboard history and recency;
- global-hotkey config state;
- Windows foreground hotkey diagnostics.
- localhost IPC server.

The default binary command initializes this state and exits. `freepalette-daemon
run` can register the configured hotkey on Windows and wait in the foreground.
That diagnostic path logs configured hotkey presses. The crate does not expose
config file watching or open/focus the UI from a separate daemon process yet.
`freepalette-daemon serve` starts a foreground local IPC server for status,
search, explicit execution, app index refresh, config reload, and shutdown
requests.
`freepalette-daemon start` launches that server in the background, and
`freepalette-daemon stop` shuts it down through IPC.

### freepalette-ui

The UI crate contains a minimal Tauri palette shell. The frontend is static
HTML, CSS, and JavaScript. It invokes Rust commands in the same process to:

- search through `PaletteState`;
- move the selected result;
- execute selected non-shell actions;
- show a small settings panel for provider, clipboard, recency, and hotkey
  state;
- manually record current text clipboard contents and poll the text clipboard
  when clipboard capture is enabled in config;
- reload config through daemon state;
- reset visible palette state.

Shell actions are shown during search and require an explicit confirmation
prompt before the UI passes them to the daemon with shell execution allowed.
On Windows, the Tauri binary also owns the configured global hotkey, a tray
icon, and launch-at-sign-in tray actions. If the tray or hotkey lifecycle is
active, Escape and window close hide the palette instead of exiting the process.
If no background lifecycle exists, Escape closes the window.

At startup the UI checks for a live daemon IPC endpoint. When one is available,
search, exact selected-result execution, provider toggles, config reload, and
clipboard history actions go through IPC. If no endpoint is available, the UI
looks for a sibling `freepalette-daemon` binary and starts it in IPC mode when
possible. If that is unavailable or fails, the UI uses in-process palette state.
The UI does not install or supervise a packaged background service yet.

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

## Daemon IPC

`freepalette-daemon serve` starts a localhost TCP server. The server
writes an endpoint file under the platform runtime or local data directory. That
file contains:

- bound localhost address;
- per-run token;
- daemon process ID.

Clients send one JSON request per connection. The token is checked before the
request is handled. The current request types are:

- status;
- search;
- execute top result;
- execute selected result;
- record or clear clipboard history;
- toggle provider and clipboard capture config;
- refresh app indexing;
- reload config;
- shutdown.

This is local IPC plumbing, not an installed system service. The protocol is not
promised stable for external integrations yet.

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
The current Tauri UI polls the text clipboard while it is running and capture is
enabled. It is not a separate background daemon.

## Ranking

Ranking is simple:

- fuzzy score over title, subtitle, and keywords;
- a small minimum fuzzy score for plain provider results, which keeps weak
  long-distance matches out of broad app searches;
- provider score hints for command-style results such as calculator and shell;
- small app score hints, currently used to demote noisy Start Menu entries such
  as uninstallers and documentation shortcuts;
- exact-title and prefix-title bonuses;
- small result-kind bias for current MVP ergonomics;
- a small local recency boost after successful non-clipboard execution;
- title and ID ordering as the final tie-breakers.

The constants are approximate. Recency is local state only. There is no
telemetry, account state, or cloud ranking.

## Windows App Indexing

The app provider scans these Start Menu roots on Windows:

- `%APPDATA%\Microsoft\Windows\Start Menu\Programs`
- `%ProgramData%\Microsoft\Windows\Start Menu\Programs`

It recursively indexes `.lnk`, `.exe`, and `.appref-ms` files. Configured apps
are loaded first and win over discovered apps with the same display name. User
Start Menu entries are checked before system entries. Configured app
`keywords` can be used as local aliases. Discovered uninstallers, help files,
documentation, manuals, readmes, and release notes remain searchable but receive
a lower score than normal app entries.

Shortcut-like entries open through the Windows shell. Direct `.exe` entries are
launched by path. If indexing is unsupported, unavailable, or empty, the
provider records that state and uses a labeled Notepad fallback only when there
are no configured apps.

## Current Limits

- App indexing is Windows-first.
- Clipboard history is local-only. The Tauri UI can manually record and poll the
  current text clipboard when capture is enabled, but encryption, source-app
  exclusions, and secret detection are not implemented. See
  [Clipboard Security Model](CLIPBOARD_SECURITY.md).
- IPC runs only when `freepalette-daemon start` or `freepalette-daemon serve`
  is used.
- Global hotkey registration in the Tauri UI is Windows-only and disabled by
  default. The foreground daemon diagnostic path remains separate. See
  [Hotkeys](HOTKEYS.md).
- Tray integration and launch-at-sign-in tray actions are Windows-only.
- Windows NSIS installer configuration exists, but installer artifacts are not
  signed or published from this repository yet.
- The UI has a basic shell-confirmation prompt, not a dedicated shell action
  review surface.
- External plugin execution is not implemented.
- The UI is usable for smoke testing but is not a finished launcher.
