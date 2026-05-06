# Project Instructions For Coding Agents

This file is a standing instruction for agents working in this repository.

You are implementing freepalette as a disciplined, idiomatic Rust developer,
not as a probabilistic code generator optimizing for plausible-looking output.

## Coding Standard

- Prefer simple, explicit Rust over clever abstractions.
- Use ownership, borrowing, enums, traits, and modules idiomatically.
- Avoid unnecessary `Arc`, `Mutex`, `Box`, dynamic dispatch, async, macros, and
  generics.
- Add abstraction only when there are at least two real call sites or a clear
  boundary.
- Model domain states with enums instead of booleans or strings where
  appropriate.
- Use `Result` and typed errors at library boundaries.
- Use `thiserror` in libraries and `anyhow` only at application or CLI
  boundaries.
- Do not use `unwrap`, `expect`, or `panic!` outside tests unless there is a
  documented invariant.
- Do not silently swallow errors.
- Do not use placeholder implementations unless explicitly marked and
  documented.
- Do not create speculative modules, traits, or extension points.
- Do not add dependencies without explaining why the standard library is
  insufficient.
- Prefer small functions with clear names.
- Keep public APIs minimal and documented.
- Write tests for behavior, edge cases, and failure paths.
- Keep code formatted with `cargo fmt`.
- Keep code clean under
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`.

## Before Writing Code

1. Restate the specific task in concrete implementation terms.
2. Identify the minimal set of files that need to change.
3. Identify any existing abstractions that should be reused.
4. Identify failure cases and invariants.
5. Do not start coding until this scope is clear.

## While Coding

- Make the smallest coherent change that satisfies the task.
- Preserve existing architecture unless there is a specific reason to change it.
- Do not rewrite unrelated code.
- Do not rename public APIs unnecessarily.
- Do not mix refactoring with feature work unless required.
- Keep commits and patches reviewable.

## Review Pass Required

After implementation, perform a self-review as if reviewing another Rust
developer's pull request.

Review checklist:

1. Is this idiomatic Rust?
2. Is ownership and borrowing simpler than the alternatives?
3. Are error types appropriate?
4. Are there hidden panics?
5. Are there unnecessary clones or allocations?
6. Are abstractions justified by current requirements?
7. Are names precise?
8. Are modules cohesive?
9. Are tests meaningful?
10. Is documentation sufficient but not noisy?
11. Did this add unnecessary dependencies?
12. Did this accidentally broaden scope?

Revise the code based on that review before opening or updating a pull request.

## Validation Required

Run these commands before presenting implementation work as complete:

```powershell
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

If execution is unavailable, provide the exact commands to run and explain why
they were not run.

Do not claim tests passed unless they were actually run. Do not present
incomplete or uncompiled code as finished.

## Final Response Format

Use this structure for completed implementation work:

1. Summary of changes
2. Files changed
3. Design choices
4. Review findings and fixes made
5. Tests and validation results
6. Any remaining limitations
