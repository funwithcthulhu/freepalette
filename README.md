# freepalette

freepalette is a local-first, open-source command palette and launcher written in Rust.

## Status

This project is early-stage. The current repository is a foundation for the core
search and provider model, not a complete desktop launcher.

## What It Is

freepalette is a fast, boring, dependable desktop utility for finding commands
and launching local actions from the keyboard. It is inspired by the general
tradition of tools like Alfred, Raycast, Microsoft PowerToys Run, Ulauncher,
Apple Spotlight, and editor command palettes.

It is not affiliated with Alfred, Raycast, Microsoft PowerToys, Ulauncher, Apple
Spotlight, or any other referenced product. It must not copy branding, names,
UI, or proprietary behavior from any specific product.

## What It Is Not

- Not an AI launcher.
- Not a cloud service.
- Not an account-based product.
- Not a telemetry collection project.
- Not a marketplace or paid-tier product.
- Not an attempt to fully replace mature launchers immediately.

## Local-First And Privacy

freepalette should work without an account, without cloud sync, and without
network access for core launcher behavior. The project does not collect
telemetry. Command execution, clipboard history, and plugins are treated as
security-sensitive areas.

## MVP Goals

- CLI search over built-in providers.
- Calculator queries such as `calc 2+2`.
- Shell command queries prefixed with `>`.
- Stub app launcher provider with sample entries.
- Stub clipboard provider with documented platform limitations.
- TOML config loading.
- Fuzzy search and simple documented ranking.
- Clear plugin boundary without external plugin execution yet.

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

Shell commands are displayed as actions by default. Use `--run` only when you
intend to execute the selected result:

```powershell
cargo run -p freepalette-cli -- search "> echo hello" --run
```

## Architecture Overview

- `freepalette-core`: config loading, provider registry, fuzzy search, ranking,
  and built-in providers.
- `freepalette-cli`: developer and user CLI for testing search and providers.
- `freepalette-daemon`: future background state for hotkeys, indexing,
  clipboard history, and provider refresh.
- `freepalette-plugin-api`: stable public API types for future plugins.
- `freepalette-ui`: placeholder crate for the eventual GUI. The likely frontend
  direction is Tauri v2 with a thin Svelte UI after the Rust core settles.

See `docs/ARCHITECTURE.md` for details.

## Contributing

Contributions should keep the project small, local-first, and easy to review.
Read `CONTRIBUTING.md`, `docs/NON_GOALS.md`, and `docs/PHILOSOPHY.md` before
larger changes.

## License

Licensed under either of:

- MIT license, see `LICENSE-MIT`
- Apache License, Version 2.0, see `LICENSE-APACHE`
