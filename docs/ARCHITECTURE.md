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

- app launcher stub with sample entries
- calculator provider for `calc` arithmetic
- shell provider for `>` commands
- clipboard history stub

### freepalette-cli

Provides commands for testing providers and search behavior without a GUI.

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

## Platform Boundaries

Platform-specific implementation should sit behind providers, daemon services,
or UI integration layers. Windows-first implementation is acceptable initially,
but core data types should stay portable.

Current limitations:

- real platform app indexing is not implemented
- clipboard capture is not implemented
- global hotkeys are not implemented
- config watching is not implemented

## Licensing Metadata

All Cargo packages use `license = "MIT OR Apache-2.0"`. The repository includes
`LICENSE-MIT` and `LICENSE-APACHE` for the dual license. A root `LICENSE` file
contains the MIT text so GitHub can show a known license in repository metadata.
