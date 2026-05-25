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

- Tauri-based palette window with a static frontend.
- Search input.
- Result list.
- Keyboard selection.
- Enter to execute selected non-shell actions.
- Escape to close.

### v0.4: Shared Daemon State

- Shared local state for config loading, provider setup, search, app index
  reporting, refresh, and action execution policy.
- CLI and UI route through that shared state.
- The daemon binary initializes local state and exits; it is not an IPC process
  yet.

### v0.5: Windows UI Lifecycle Groundwork

- Clipboard security model.
- In-memory clipboard history state with capture off by default.
- Clipboard retention limits and clear behavior in daemon state.
- Hotkey config validation and platform status reporting.
- Foreground Windows hotkey registration through `freepalette-daemon run`.
- UI shell execution refusal with no shell confirmation flow yet.
- Windows hotkey, tray, and launch-at-sign-in helper code wired into the current
  Tauri binary.
- Checked-in palette-and-brush icon assets.
- Windows daily-use and UI smoke-test documentation.
- Tauri UI shell for the current Windows desktop path.

## Next

- Improve CLI and provider documentation as behavior changes.
- Add release packaging notes and installer work for Windows.
- Add explicit user controls before any real system clipboard capture.
- Add a CLI command for app index refresh if a long-running process needs it.
- Keep shell execution blocked in UI until a confirmation flow exists.
- Add basic app icons/metadata to indexed app results where platform data is
  available.
- Embed the checked-in Windows icon when packaging work starts.

## Later

- Long-running daemon process.
- IPC integration for a separate daemon if the single-process UI path is not
  enough.
- System clipboard capture.
- Clipboard persistence after storage location and deletion behavior are
  documented.
- Better app launch metadata and icons.
- macOS and Linux tray/autostart equivalents if the UI lifecycle model holds up.
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
