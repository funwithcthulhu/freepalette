# Changelog

All notable changes to this project will be documented in this file.

The format is based on manual release notes. The project intends to follow
semantic versioning once releases begin, with normal 0.x API instability while
the core model settles.

## Unreleased

- Added a local daemon/service state layer for shared config loading, provider
  registry setup, app index refresh, search, and action execution policy.
- Routed the CLI and minimal UI through the shared daemon state instead of
  constructing provider registries separately.
- Added CLI integration tests for calculator search, shell search, shell
  execution gating, and disabled app-provider reporting.
- Improved the CLI shell-blocked error to point users at `--allow-shell`.
- Added development documentation and tightened security/plugin/non-goal docs
  around the daemon boundary.
- Named ranking and provider score constants, and added tests for ranking
  tie-breaking, config defaults, and empty calculator/shell query parsing.
- Replaced internal stringly app-index errors with explicit unsupported-platform
  and missing-Windows-environment states.
- Renamed the plugin API query wrapper from `Query` to `SearchQuery`.
- Rewrote README, architecture, roadmap, contributing, and development docs to
  describe current behavior and limitations more directly.

## 0.3.0 - 2026-05-06

- Bumped workspace crate versions to `0.3.0`.
- Removed the duplicate root `LICENSE` file and restored the standard
  dual-license layout with `LICENSE-APACHE` and `LICENSE-MIT`.
- Added the first minimal desktop UI in `freepalette-ui` using `eframe`/`egui`,
  with search input, result list, keyboard selection, Enter execution, and
  Escape close.

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
