# GitHub Settings Checklist

## Repository

- Visibility: Public
- Default branch: main
- Issues: enabled
- Discussions: optional, recommended after first usable release
- Wiki: disabled initially
- Projects: optional
- Sponsorships: disabled initially unless intentionally configured

## Pull Requests

- Allow squash merging: enabled
- Allow merge commits: disabled
- Allow rebase merging: recommend disabled initially for simplicity
- Always suggest updating pull request branches: enabled
- Automatically delete head branches: enabled
- Require contributors to sign off: disabled unless DCO is intentionally adopted

## Branch Protection Or Ruleset For `main`

GitHub branch protection or repository rulesets can require pull requests,
reviews, status checks, linear history, and can block deletion or force-pushes.

Recommended `main` protection:

- Require a pull request before merging: enabled
- Required approvals: 1
- Dismiss stale pull request approvals when new commits are pushed: enabled
- Require review from Code Owners: disabled initially unless `CODEOWNERS` exists
- Require status checks to pass before merging: enabled
- Required checks: `fmt`, `clippy`, `test`
- Require branches to be up to date before merging: enabled if CI is fast,
  otherwise optional
- Require conversation resolution before merging: enabled
- Require linear history: enabled if using squash-only merges
- Do not allow force pushes
- Do not allow deletions
- Include administrators: recommend enabled after CI is stable, disabled during
  the first setup day if needed

## Security Settings

- Dependabot alerts: enabled
- Dependabot security updates: enabled
- Secret scanning: enabled if available
- Push protection: enabled if available
- Code scanning: optional initially

## Labels

Recommended labels:

- bug
- enhancement
- documentation
- good first issue
- help wanted
- question
- scope: core
- scope: cli
- scope: ui
- scope: daemon
- scope: plugin-api
- security
- needs design

## Topics

Recommended topics:

- rust
- launcher
- command-palette
- productivity
- desktop
- local-first
- open-source
- cli

## Initial Description

`A local-first, open-source command palette and launcher written in Rust.`
