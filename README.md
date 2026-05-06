# freepalette

freepalette is a local-first, open-source command palette and launcher written in Rust.

## Status

freepalette is early-stage. The repository currently provides a Rust core,
provider model, CLI, documentation, and project infrastructure. It is not yet a
complete desktop launcher.

## What It Is

freepalette is a keyboard-first desktop utility for searching local commands and
launching explicit local actions. It is inspired by the general tradition of
desktop launchers and command palettes, including Alfred, Raycast, Microsoft
PowerToys Run, Ulauncher, Apple Spotlight, and editor command palettes.

freepalette is not affiliated with Alfred, Raycast, Microsoft PowerToys,
Ulauncher, Apple Spotlight, or any other referenced product. It must not copy
branding, names, UI, or proprietary behavior from any specific product.

## What It Is Not

- Not an AI launcher.
- Not a cloud service.
- Not an account-based product.
- Not a telemetry collection project.
- Not a marketplace or paid-tier product.
- Not an attempt to fully replace mature launchers immediately.

## Local-First And Privacy

Core launcher behavior should work without an account, cloud service, or network
connection. freepalette does not collect telemetry. Command execution, clipboard
history, and future plugins are treated as security-sensitive areas.

## Current MVP

Implemented now:

- CLI search over built-in providers.
- Calculator queries prefixed with `calc`, such as `calc 2+2`.
- Shell command queries prefixed with `>`, displayed as actions by default.
- Stub app launcher provider with sample entries.
- Stub clipboard provider.
- TOML config loading from a path.
- Fuzzy search and simple documented ranking.
- Provider/action API boundary for built-in providers and future plugin design.

Intentionally not implemented yet:

- real platform app indexing
- clipboard capture or persistence
- global hotkey daemon behavior
- GUI beyond a placeholder crate
- external plugin execution

## Build

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

## CLI Examples

```powershell
cargo run -p freepalette-cli -- search "calc 2+2"
cargo run -p freepalette-cli -- search "> echo hello"
cargo run -p freepalette-cli -- search "notepad"
cargo run -p freepalette-cli -- providers
```

Shell command search returns an action. It does not execute by default. Use
`--run` only when you intend to execute the selected result:

```powershell
cargo run -p freepalette-cli -- search "> echo hello" --run
```

## Config

See `examples/config/freepalette.toml`.

The CLI can print the default future config path for the current platform:

```powershell
cargo run -p freepalette-cli -- config-path
```

## Architecture Overview

- `freepalette-core`: config loading, provider registry, fuzzy search, ranking,
  and built-in providers.
- `freepalette-cli`: developer and user CLI for testing search and providers.
- `freepalette-daemon`: placeholder for future hotkeys, indexing, clipboard
  state, config reload, and provider refresh.
- `freepalette-plugin-api`: public provider and action API types.
- `freepalette-ui`: placeholder crate for the eventual GUI. The likely frontend
  direction is Tauri v2 with a thin Svelte UI after the Rust core settles.

See `docs/ARCHITECTURE.md` for details.

## Documentation

- `docs/PHILOSOPHY.md`: project values and constraints.
- `docs/NON_GOALS.md`: what the MVP intentionally excludes.
- `docs/PLUGIN_MODEL.md`: plugin options and current recommendation.
- `docs/ROADMAP.md`: staged implementation milestones.
- `docs/GITHUB_SETTINGS.md`: repository settings checklist and current setup.
- `docs/RELEASES.md`: release process.
- `docs/GOVERNANCE.md`: maintainer-led governance.

## Contributing

Contributions should keep the project small, local-first, no-telemetry, and easy
to review. Read `CONTRIBUTING.md`, `docs/NON_GOALS.md`, and
`docs/PHILOSOPHY.md` before larger changes.

## License

freepalette uses the standard Rust ecosystem dual license:

- MIT license, see `LICENSE-MIT`
- Apache License, Version 2.0, see `LICENSE-APACHE`

Cargo package metadata is set to `MIT OR Apache-2.0`.

The root `LICENSE` file contains the MIT license text so GitHub can display a
known license in the repository sidebar. The full project licensing choice
remains `MIT OR Apache-2.0`.
