# Contributing

freepalette is maintainer-led and early. Small, focused pull requests are easier
to review than broad rewrites.

## Before You Start

- Open an issue for bugs and small feature requests.
- Open a design issue before changing provider APIs, execution behavior,
  plugin direction, app indexing, or security-sensitive code.
- Check [NON_GOALS.md](docs/NON_GOALS.md) before proposing larger features.

## Local Checks

Run these before opening a pull request:

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

Useful commands while working:

```powershell
cargo run -p freepalette-cli -- providers
cargo run -p freepalette-cli -- search "calc 2+2"
cargo run -p freepalette-cli -- search "> echo hello"
cargo run -p freepalette-cli -- apps list
```

## Pull Requests

Good pull requests usually include:

- a short explanation of the behavior change;
- tests for the changed behavior;
- docs updates when user-visible behavior changes;
- notes about shell, clipboard, app launch, or plugin security impact.

Avoid mixing refactors with feature work unless the refactor is needed for the
feature and stays small.

## Provider Changes

Provider work belongs in `freepalette-core` unless it is only CLI or UI output.
A provider should make these things clear:

- which queries it recognizes;
- which `SearchResult` values it returns;
- which `Action` values it can execute;
- what happens on unsupported platforms;
- what is stubbed or intentionally missing.

Do not add network calls, telemetry, accounts, or background cloud dependencies
to MVP providers.

## Security-Sensitive Code

Treat these areas as security-sensitive:

- shell command execution;
- app launching;
- clipboard reads and writes;
- config loading and future config watching;
- future plugin execution.

Shell actions must require explicit execution and the `--allow-shell` CLI guard.
Search must not run shell commands.

## Licensing

New code is contributed under `MIT OR Apache-2.0`, matching the workspace Cargo
metadata. Keep `LICENSE-MIT`, `LICENSE-APACHE`, README, and release docs in
sync if licensing metadata changes.
