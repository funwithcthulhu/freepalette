# freepalette

freepalette is an early Rust command palette and app launcher.

It is local-first, has no account system, and does not collect telemetry. The
project is in the same broad category as desktop launchers and editor command
palettes, but it is not affiliated with Alfred, Raycast, Microsoft PowerToys,
Ulauncher, Apple Spotlight, or any other referenced tool.

## Status

This is not a complete desktop launcher yet. The current repo has a working Rust
core, a CLI, Windows Start Menu app indexing, a minimal Tauri UI shell, and early
daemon/plugin-facing crates.

## What Works

- CLI search over built-in providers.
- Calculator queries prefixed with `calc`, for example `calc 2+2`.
- Shell command queries prefixed with `>`. Search displays the action but does
  not run it.
- Windows Start Menu app indexing.
- App index inspection with `apps list` and `debug apps`.
- Explicit top-result execution with `run`.
- TOML config loading from an explicit path or the platform default location.
- Fuzzy search plus a small ranking model.
- A minimal Tauri desktop UI in `freepalette-ui`.
- On Windows, the Tauri UI wires the configured hotkey, tray icon, and
  launch-at-sign-in menu actions into the single-process UI lifecycle.
- Clipboard provider backed by explicit in-memory daemon state. System clipboard
  capture and persistence are not implemented.
- Hotkey config validation in daemon state.
- Foreground Windows hotkey registration with `freepalette-daemon run` for
  diagnostics. That path logs presses but does not open the UI.

## What Does Not Work Yet

- A long-running IPC daemon.
- macOS or Linux global hotkey registration.
- macOS or Linux tray integration or autostart setup.
- Clipboard capture or persistence.
- External plugin execution.
- macOS or Linux app indexing.
- Windows installer packaging.
- A polished shell-confirmation UI in the desktop app.
- A polished desktop launcher experience.

## Build And Test

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

## CLI

```powershell
cargo run -p freepalette-cli -- search "calc 2+2"
cargo run -p freepalette-cli -- search "> echo hello"
cargo run -p freepalette-cli -- search "notepad"
cargo run -p freepalette-cli -- apps list
cargo run -p freepalette-cli -- debug apps
cargo run -p freepalette-cli -- providers
cargo run -p freepalette-cli -- config-path
```

Run the top result only when you mean to execute it:

```powershell
cargo run -p freepalette-cli -- run "notepad"
```

Shell actions require an extra flag:

```powershell
cargo run -p freepalette-cli -- run "> echo hello" --allow-shell
```

The development-only `search --run` path follows the same shell rule:

```powershell
cargo run -p freepalette-cli -- search "> echo hello" --run --allow-shell
```

## Config

See [examples/config/freepalette.toml](examples/config/freepalette.toml).

The CLI accepts `--config <path>` for commands that load providers.

Clipboard capture is off by default:

```toml
[clipboard]
capture = false
max_entries = 50
max_entry_bytes = 4096
```

The daemon has in-memory clipboard-history state for future capture work, but
it does not watch the system clipboard or write clipboard history to disk.

The hotkey config is also disabled by default:

```toml
[hotkey]
enabled = false
key = "Space"
ctrl = true
alt = true
```

On Windows, the Tauri UI reads this config at startup. If the binding is
enabled and can be registered, pressing it shows and focuses the palette. The
hotkey remains disabled by default.

The daemon can also register the same binding in a foreground diagnostic mode:

```powershell
cargo run -p freepalette-daemon -- run
```

With the default config this command exits because the hotkey is disabled. When
enabled on Windows, it registers the configured binding and waits in the
foreground. This diagnostic daemon path logs presses but does not open or focus
the UI.

## Windows App Indexing

The app provider scans these Start Menu locations when they exist:

- `%APPDATA%\Microsoft\Windows\Start Menu\Programs`
- `%ProgramData%\Microsoft\Windows\Start Menu\Programs`

It indexes `.lnk`, `.exe`, and `.appref-ms` files. Shortcut-like entries open
through the Windows shell; direct `.exe` entries launch by path.
Configured app entries win over discovered entries with the same display name.

When indexing is unavailable or empty, the provider records that state and uses
a clearly labeled Notepad fallback only when there are no configured apps.

## Crates

- `freepalette-core`: config, providers, fuzzy search, ranking, app indexing,
  and action dispatch.
- `freepalette-cli`: command-line search, inspection, and explicit run support.
- `freepalette-daemon`: shared local state for config loading, provider setup,
  search, app index reports, refresh, in-memory clipboard history, hotkey
  config state, Windows foreground hotkey diagnostics, and action execution
  policy. It is not an IPC daemon yet.
- `freepalette-plugin-api`: public provider/action data types used by built-in
  providers and future plugin protocol work.
- `freepalette-ui`: minimal Tauri palette shell with a static frontend over the
  Rust palette state. On Windows it owns the configured hotkey, tray icon, and
  launch-at-sign-in tray actions. It is early and has no daemon IPC, installer,
  or polished shell-confirmation UI.

## Security-Sensitive Areas

Treat these areas carefully in issues and pull requests:

- shell command execution;
- app launching;
- clipboard history and clipboard writes;
- config loading and future file watching;
- future plugin execution.

Shell commands must not execute from search alone. In the UI, shell actions
require an explicit confirmation prompt before they are passed to the daemon
with shell execution allowed. External plugin execution is not implemented.

## Documentation

- [Architecture](docs/ARCHITECTURE.md)
- [Clipboard security](docs/CLIPBOARD_SECURITY.md)
- [Development](docs/DEVELOPMENT.md)
- [Hotkeys](docs/HOTKEYS.md)
- [Windows daily use](docs/WINDOWS_DAILY_USE.md)
- [UI smoke test checklist](docs/UI_SMOKE_TEST.md)
- [Roadmap](docs/ROADMAP.md)
- [Non-goals](docs/NON_GOALS.md)
- [Plugin model](docs/PLUGIN_MODEL.md)
- [Security](SECURITY.md)
- [Contributing](CONTRIBUTING.md)

## License

Licensed under either of:

- Apache License, Version 2.0, see [LICENSE-APACHE](LICENSE-APACHE)
- MIT license, see [LICENSE-MIT](LICENSE-MIT)

Cargo package metadata is set to `MIT OR Apache-2.0`.
