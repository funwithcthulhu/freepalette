# Philosophy

freepalette exists to be a small, local-first launcher that respects user
control.

## Local-First

Core launcher behavior should work without a network connection, account, cloud
sync, or remote service.

## Low-Friction

The tool should be quick to start, easy to configure, and simple to understand.
The CLI should remain useful for debugging even after a GUI exists.

## Dependable Over Flashy

Predictable behavior matters more than novelty. The project should prefer clear
features that can be tested and maintained.

## Keyboard-First

freepalette should optimize for users who want to quickly search, select, and
act from the keyboard.

## Boring Maintainable Rust

The Rust code should be explicit and conservative. Abstractions should earn
their place by reducing real complexity.

## User Control

Actions such as shell command execution, clipboard writes, app launching, and
future plugin execution require clear user intent.

## No Telemetry

The project must not collect telemetry. Diagnostics should be opt-in and local.

## No Cloud Dependency

Cloud features are not part of the MVP. Core behavior should not depend on
hosted services.

## Not A Clone

freepalette belongs to a broad product category, but it should not copy the
branding, names, UI, or proprietary behavior of any specific launcher or command
palette.
