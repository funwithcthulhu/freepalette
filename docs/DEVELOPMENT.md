# Development

freepalette is a Rust workspace. Keep changes narrow and make behavior visible
through tests or CLI output.

## Branch Workflow

```powershell
git checkout main
git pull --ff-only
git checkout -b codex/short-description
```

## Checks

```powershell
cargo fmt --all
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

## Crate Responsibilities

- `freepalette-core`: config, provider registry, providers, fuzzy search,
  ranking, app indexing, and action dispatch.
- `freepalette-daemon`: shared local daemon state. It is not a background IPC
  process yet.
- `freepalette-cli`: command parsing and terminal output.
- `freepalette-ui`: minimal egui palette state and window.
- `freepalette-plugin-api`: provider/action data types.

## Adding A Provider

1. Add the provider under `crates/freepalette-core/src/providers`.
2. Keep platform-specific code inside that provider or a small child module.
3. Register the provider in `providers/mod.rs`.
4. Add config flags only when users should be able to disable it.
5. Test query recognition, returned actions, and execution behavior.
6. Document unsupported platforms and stubbed behavior.

Providers return candidate actions during search. They should execute only after
the selected action comes back through the registry.

## CLI Tests

CLI integration tests live in `crates/freepalette-cli/tests`. Prefer temporary
config files that disable unrelated providers. That keeps tests independent of
the local app index.

## Documentation

Update docs in the same pull request when behavior changes. Keep limitations
visible. Do not document planned platform support or plugin support as if it
already exists.
