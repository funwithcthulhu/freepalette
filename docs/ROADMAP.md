# Roadmap

This roadmap separates shipped work from planned work. Items may move as the
project settles.

## Implemented

### v0.1: CLI Search + Built-In Provider Skeleton

- Rust workspace foundation.
- CLI search command.
- TOML config loading.
- Built-in calculator, shell, app stub, and clipboard stub providers.
- Fuzzy matching and simple ranking.
- CI for fmt, clippy, and tests.

### v0.2: Windows App Indexing

- Windows Start Menu scanning for `.lnk`, `.exe`, and `.appref-ms` entries.
- Windows shell opening for shortcut-like Start Menu entries.
- Fallback behavior when indexing is unavailable or empty.
- CLI app index inspection.
- Explicit CLI execution with `freepalette run`.
- Shell execution guard with `--allow-shell`.

### v0.3: Minimal UI

- egui-based palette window.
- Search input.
- Result list.
- Keyboard selection.
- Enter to execute selected non-shell actions.
- Escape to close.

### v0.4 Partial: Shared Daemon State

- Shared local state for config loading, provider setup, search, app index
  reporting, refresh, and action execution policy.
- CLI and UI route through that shared state.

## Next

- Improve CLI and provider documentation as behavior changes.
- Add a CLI command for app index refresh if a long-running process needs it.
- Decide the smallest Windows global-hotkey path.
- Keep shell execution blocked in UI until a confirmation flow exists.

## Later

- Long-running daemon process.
- Windows global hotkey registration.
- Clipboard capture and local retention.
- Better app launch metadata and icons.
- macOS and Linux app indexing.
- Subprocess plugin protocol.
- Packaging notes for each platform.

## Out Of Scope For Now

- Accounts.
- Cloud sync.
- Telemetry.
- Paid tier.
- Plugin marketplace.
- External plugin execution before a security model exists.
- Mobile app.
- AI assistant behavior.
