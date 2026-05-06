# Contributing

freepalette is maintainer-led and intentionally narrow. Contributions are
welcome when they preserve the local-first, no-telemetry direction of the
project.

## Expected Workflow

1. Open an issue for bugs or small feature requests.
2. Open an RFC-style issue before large design changes.
3. Keep pull requests focused.
4. Update docs when behavior, configuration, architecture, or security posture
   changes.

## Build

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

Useful local commands:

```powershell
cargo run -p freepalette-cli -- providers
cargo run -p freepalette-cli -- search "calc 2+2"
cargo run -p freepalette-cli -- search "> echo hello"
cargo run -p freepalette-cli -- search "notepad"
```

## Filing Issues

Bug reports should include the OS, freepalette version or commit, expected
behavior, actual behavior, reproduction steps, and logs or command output.

Feature requests should explain the problem, a proposed solution, alternatives,
why the request fits project scope, and privacy or security implications.

## Coding Style

- Prefer small modules and explicit data flow.
- Use `thiserror` for library errors and `anyhow` at CLI or application
  boundaries.
- Avoid `unwrap` and `expect` outside tests or clearly justified startup code.
- Keep provider behavior testable without a GUI.
- Use `tracing` for structured logging.
- Do not add telemetry.

## Adding Providers

Providers should implement the `Provider` trait from `freepalette-plugin-api`.
New providers should include:

- clear query detection rules
- meaningful result titles and actions
- tests for matching behavior
- documentation for platform limitations
- no background network dependency for MVP behavior

## Keeping Scope Narrow

Do not add cloud sync, accounts, AI assistant behavior, plugin marketplace
features, or automatic shell command execution. If a feature changes the
security model, start with a design issue before code.
