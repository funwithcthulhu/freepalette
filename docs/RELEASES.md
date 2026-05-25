# Releases

freepalette intends to follow semantic versioning once releases begin.

During 0.x, APIs may change while the core provider model settles. Breaking
changes should still be documented.

## Changelog

`CHANGELOG.md` is maintained manually.

## Licensing

Crates should keep `license = "MIT OR Apache-2.0"`. The repository keeps
`LICENSE-MIT` and `LICENSE-APACHE`.

## Release Checklist

1. Run `cargo fmt --all -- --check`.
2. Run `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
3. Run `cargo test --workspace --all-features`.
4. Run the UI smoke-test checklist when UI lifecycle behavior changed.
5. For a Windows UI build, run
   `cargo tauri build --bundles nsis --ci` from `crates/freepalette-ui`.
6. Record whether the Windows installer is unsigned.
7. Update `CHANGELOG.md`.
8. Tag the release.
9. Publish crates only when the API is useful and stable enough.

Do not attach installer artifacts to a release as official builds until signing
and release provenance are documented.
