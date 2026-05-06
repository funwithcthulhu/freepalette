# Architecture

freepalette is a Rust workspace with a small core and thin outer crates.

The current architecture is Rust-core-first. The GUI is intentionally deferred
until search, provider behavior, config loading, and security boundaries are
clear enough to support a thin frontend.

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

Placeholder for future long-running state:

- global hotkey registration
- app indexing
- clipboard history
- config reloads
- provider refresh

### freepalette-ui

Placeholder crate for the future GUI. The likely direction is Tauri v2 with a
thin Svelte frontend after the Rust core API settles.

## Search Flow

1. Load `Config`.
2. Register enabled providers in `ProviderRegistry`.
3. Wrap raw input in `Query` and `SearchContext`.
4. Ask each provider for candidate `SearchResult` values.
5. Rank candidates with fuzzy score, score hints, exact/prefix bonuses, and a
   small result-kind bias.
6. Display results in the CLI, daemon client, or future UI.
7. Execute an `Action` only after explicit user selection.

The CLI exposes app-index inspection through `freepalette apps list` and
`freepalette debug apps`. Both commands use the app provider's debug report
rather than duplicating platform indexing logic in the CLI.

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
