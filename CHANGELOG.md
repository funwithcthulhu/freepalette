# Changelog

All notable changes to this project will be documented in this file.

The format is based on manual release notes. The project intends to follow
semantic versioning once releases begin, with normal 0.x API instability while
the core model settles.

## Unreleased

- Created initial Rust workspace skeleton.
- Added core provider registry, fuzzy search, ranking, and TOML config loading.
- Added built-in calculator, shell, app stub, and clipboard stub providers.
- Added CLI search and provider listing commands.
- Added daemon and UI placeholder crates.
- Added documentation, CI, and GitHub community files.
- Added root `LICENSE` for GitHub license detection while keeping the dual
  `MIT OR Apache-2.0` project license.
- Added Windows Start Menu app indexing behind the app provider, with fallback
  behavior when indexing is unavailable or empty, plus a small Windows built-in
  Notepad seed for the basic launcher demo.
