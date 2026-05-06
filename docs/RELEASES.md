# Releases

freepalette intends to follow semantic versioning once releases begin.

During 0.x, APIs may change while the core provider model settles. Breaking
changes should still be documented.

## Changelog

`CHANGELOG.md` is maintained manually.

## Licensing

Crates should keep `license = "MIT OR Apache-2.0"`. The repository keeps
`LICENSE-MIT`, `LICENSE-APACHE`, and a root `LICENSE` file for GitHub license
detection.

## Release Checklist

1. Run `cargo fmt --all -- --check`.
2. Run `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
3. Run `cargo test --workspace --all-features`.
4. Update `CHANGELOG.md`.
5. Tag the release.
6. Publish crates only when the API is useful and stable enough.
