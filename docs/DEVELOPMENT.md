# Development

freepalette is intentionally small and Rust-first. Prefer straightforward code
that keeps provider behavior, daemon state, CLI behavior, and UI behavior easy
to test independently.

## Local Workflow

Start from an up-to-date `main` branch and make focused branches:

```powershell
git checkout main
git pull --ff-only
git checkout -b codex/short-description
```

Before opening a pull request, run:

```powershell
cargo fmt --all
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

## Crate Responsibilities

- `freepalette-core`: config, provider registry, fuzzy search, ranking,
  built-in providers, and action dispatch.
- `freepalette-daemon`: shared local service state for config loading, provider
  setup, search, app index reporting, refresh, and action execution policy.
- `freepalette-cli`: command-line surface for search, debugging, and explicit
  execution.
- `freepalette-ui`: minimal desktop palette state and egui shell.
- `freepalette-plugin-api`: public data types for provider and future plugin
  protocol boundaries.

## Adding A Provider

1. Add the provider implementation under `freepalette-core/src/providers`.
2. Keep provider-specific platform code inside the provider module or a narrow
   helper module.
3. Register the provider in `freepalette-core/src/providers/mod.rs`.
4. Add config flags only when the provider should be user-toggleable.
5. Add behavior tests for matching, ranking impact, and action execution.
6. Add CLI or daemon tests when the provider affects user-visible commands.

Providers should return candidate actions during search and execute only after
explicit user selection. Shell commands, clipboard writes, app launches, and
future plugin actions need especially careful tests.

## CLI Tests

CLI integration tests live under `crates/freepalette-cli/tests`. Prefer test
configs that disable unrelated providers so tests do not depend on the local
machine's app index.

## Documentation

Update docs in the same pull request when behavior changes. Keep documents
plain, current, and explicit about limitations. Do not document future platform
support as if it already exists.
