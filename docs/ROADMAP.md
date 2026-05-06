# Roadmap

## v0.1: CLI Search + Built-In Provider Skeleton

- Rust workspace foundation.
- CLI search command.
- TOML config loading.
- Built-in calculator, shell, app stub, and clipboard stub providers.
- Fuzzy matching and simple ranking.
- CI for fmt, clippy, and tests.

## v0.2: Real App Indexing On Windows

- Discover Start Menu applications on Windows.
- Keep app indexing behind provider boundaries.
- Preserve clear fallback behavior when indexing is unavailable.

Initial implementation is present. Future work can improve shortcut metadata,
icons, app refresh behavior, and Windows-specific launch fidelity.

## v0.3: Minimal GUI

- Choose the GUI shell.
- Build a thin frontend over the Rust core, likely Tauri v2 and Svelte if that
  remains the simplest maintainable path.
- Support search input, result list, keyboard navigation, and explicit action.

## v0.4: Global Hotkey + Daemon

- Add a background daemon.
- Register global hotkey on Windows.
- Provide a path for config reload and provider refresh.

## v0.5: Clipboard History

- Capture clipboard changes locally.
- Add configurable retention.
- Avoid logging clipboard contents by default.

## v0.6: Subprocess Plugin Protocol

- Define JSON-RPC/stdin-stdout protocol.
- Add plugin manifests.
- Add timeouts and structured errors.
- Provide one example external plugin.

## v0.7: macOS/Linux Parity Work

- App indexing on macOS and Linux.
- Hotkey integration on macOS and Linux.
- Packaging notes for each platform.
