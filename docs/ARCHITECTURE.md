# Architecture

freepalette is a Rust workspace with a small core and thin outer crates.

The current architecture is Rust-core-first. The GUI and CLI are intentionally
thin and use the same local daemon/service state for config loading, provider
setup, search, ranking, app index refresh, and action execution policy.

## Crates

### freepalette-plugin-api

Defines stable public API types for providers and future plugin protocols:

- `Query`
- `SearchContext`
- `SearchResult`
- `Action`
- `ActionOutcome`
- `Provider`

The Rust trait is for built-in providers and internal extension points. External
plugins should use a serialized protocol instead of relying on Rust trait-object
ABI stability.

### freepalette-core

Owns UI-independent behavior:

- TOML config loading
- provider registration
- built-in providers
- fuzzy matching
- result ranking
- action dispatch

Built-in providers currently include:

- app launcher provider with Windows Start Menu indexing and sample fallback
- calculator provider for `calc` arithmetic
- shell provider for `>` commands
- clipboard history stub

### freepalette-cli

Provides commands for testing providers, search behavior, indexed apps, and
explicit action execution without a GUI.

### freepalette-daemon

Owns local application state that should be shared by the CLI, UI, and future
long-running daemon process:

- config source and reload behavior
- provider registry creation
- app index report and refresh
- search over the configured providers
- action execution policy, including shell-command gating
- clipboard history stub state

This crate is not an IPC service yet. The current binary only initializes the
state and reports that long-running hotkey, IPC, and clipboard capture behavior
are not implemented.

### freepalette-ui

Minimal desktop palette built with `eframe`/`egui`. It owns only UI state:
search text, ranked results, selected row, and status messages. Search and
execution go through `freepalette-daemon`, which owns provider setup and action
policy.

The first GUI pass uses `eframe` because the standard library has no desktop
windowing or widget layer, and a Rust-native dependency keeps the build smaller
than introducing a web frontend and Node toolchain at this stage. A future
Tauri frontend remains possible if it becomes the simpler long-term path.

## Search Flow

1. Load `Config` through `DaemonState`.
2. Register enabled providers in `ProviderRegistry` through the daemon state.
3. Wrap raw input in `Query` and `SearchContext`.
4. Ask each provider for candidate `SearchResult` values.
5. Rank candidates with fuzzy score, score hints, exact/prefix bonuses, and a
   small result-kind bias.
6. Display results in the CLI, daemon client, or UI.
7. Execute an `Action` only after explicit user selection.

The CLI exposes app-index inspection through `freepalette apps list` and
`freepalette debug apps`. Both commands use the daemon state's app index report
and refresh path rather than duplicating platform indexing logic in the CLI.

The UI follows the same search and execution path through the daemon state. It
does not talk to a separate long-running daemon process yet.

## Ranking Model

The ranking model is intentionally small:

- fuzzy match over title, subtitle, and keywords
- provider score hint for dynamic results such as calculator and shell commands
- exact title bonus
- prefix title bonus
- small result-kind bias for MVP usability

There is no personalization, usage tracking, telemetry, account state, or cloud
ranking.

## Execution Model

Search and execution are separate. Providers return `SearchResult` values with
an `Action`, but the core should only execute an action after explicit user
selection. This is especially important for shell commands, clipboard writes,
app launching, and future plugins.

The CLI's `run` command executes the top ranked result for a query. Shell
actions are additionally gated behind `--allow-shell`, including when using the
developer-oriented `search --run` path.

The UI also uses the daemon state's shell-command gate. Shell results can be
found, but executing one from the minimal UI is blocked until a deliberate UI
confirmation flow exists.

## Platform Boundaries

Platform-specific implementation should sit behind providers, daemon services,
or UI integration layers. Windows-first implementation is acceptable initially,
but core data types should stay portable.

Current limitations:

- app indexing is Windows-first and currently based on Start Menu entries
- macOS and Linux app indexing are not implemented
- clipboard capture is not implemented
- global hotkeys are not implemented
- config watching is not implemented
- the GUI has no tray behavior, IPC daemon integration, or global hotkey yet

## Windows App Indexing

The app provider currently indexes these Windows Start Menu roots when they are
available:

- `%APPDATA%\Microsoft\Windows\Start Menu\Programs`
- `%ProgramData%\Microsoft\Windows\Start Menu\Programs`

It scans recursively for `.lnk`, `.exe`, and `.appref-ms` files. Configured app
entries are loaded first and win over discovered entries with the same display
name. Shortcuts and ClickOnce references launch through `explorer.exe`; direct
executables launch by path.

When the same app name is discovered from multiple Start Menu roots, the earlier
root wins. This lets user-level Start Menu entries take precedence over
system-level entries before the final display list is sorted by app name.

The provider also seeds a small Windows built-in Notepad entry. This keeps the
basic `freepalette search "notepad"` demo dependable on Windows machines where
Notepad is available as `notepad.exe` but not present as a Start Menu shortcut.

If indexing is unavailable or finds no entries, the provider records that state
and uses a clearly labeled sample Notepad fallback only when there are no
configured apps.

## Licensing Metadata

All Cargo packages use `license = "MIT OR Apache-2.0"`. The repository includes
`LICENSE-MIT` and `LICENSE-APACHE` for the dual license.
