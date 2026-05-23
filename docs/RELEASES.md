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
5. Update `CHANGELOG.md`.
6. Tag the release.
7. Publish crates only when the API is useful and stable enough.

## Next Candidate

The current unreleased work is enough for a small `0.5.0` candidate once it is
verified: Windows hotkey/UI lifecycle work, tray controls, launch at sign-in,
and the first checked-in app icon assets.
