# freepalette

freepalette is an early Rust command palette and app launcher.

It is local-first, has no account system, and does not collect telemetry. The
project is in the same broad category as desktop launchers and editor command
palettes, but it is not affiliated with Alfred, Raycast, Microsoft PowerToys,
Ulauncher, Apple Spotlight, or any other referenced tool.

## Status

This is not a complete desktop launcher yet. The current repo has a working Rust
core, a CLI, Windows Start Menu app indexing, a small egui UI crate, and early
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
- A minimal desktop UI in `freepalette-ui`.
- Stub clipboard provider.

## What Does Not Work Yet

- Global hotkey registration.
- A long-running IPC daemon.
- Clipboard capture or persistence.
- External plugin execution.
- macOS or Linux app indexing.
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
  search, app index reports, refresh, and action execution policy. It is not an
  IPC daemon yet.
- `freepalette-plugin-api`: public provider/action data types used by built-in
  providers and future plugin protocol work.
- `freepalette-ui`: minimal egui palette. It is early and has no hotkey, tray,
  or daemon IPC.

## Security-Sensitive Areas

Treat these areas carefully in issues and pull requests:

- shell command execution;
- app launching;
- clipboard history and clipboard writes;
- config loading and future file watching;
- future plugin execution.

Shell commands must not execute from search alone. External plugin execution is
not implemented.

## Documentation

- [Architecture](docs/ARCHITECTURE.md)
- [Development](docs/DEVELOPMENT.md)
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
