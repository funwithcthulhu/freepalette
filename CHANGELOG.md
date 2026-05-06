# Changelog

All notable changes to this project will be documented in this file.

The format is based on manual release notes. The project intends to follow
semantic versioning once releases begin, with normal 0.x API instability while
the core model settles.

## Unreleased

## 0.2.0 - 2026-05-06

- Bumped workspace crate versions to `0.2.0`.
- Added Windows Start Menu app indexing behind the app provider, with fallback
  behavior when indexing is unavailable or empty, plus a small Windows built-in
  Notepad seed for the basic launcher demo.
- Added app index inspection through `freepalette apps list` and
  `freepalette debug apps`.
- Added `freepalette run <query>` for explicit execution of the top ranked
  result.
- Added `--allow-shell` gating for shell command execution through the CLI.
- Improved duplicate handling for discovered Windows app entries so earlier
  indexing roots win before final display sorting.

## 0.1.0 - 2026-05-05

- Created initial Rust workspace skeleton.
- Added core provider registry, fuzzy search, ranking, and TOML config loading.
- Added built-in calculator, shell, app stub, and clipboard stub providers.
- Added CLI search and provider listing commands.
- Added daemon and UI placeholder crates.
- Added documentation, CI, and GitHub community files.
- Added root `LICENSE` for GitHub license detection while keeping the dual
  `MIT OR Apache-2.0` project license.
