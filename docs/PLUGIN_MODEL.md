# Plugin Model

freepalette does not execute third-party plugins yet. The current repository
defines the boundary but keeps external execution out of scope until the
security model is documented.

## 1. Built-In Rust Providers

Built-in providers implement the `Provider` trait from `freepalette-plugin-api`.
This is appropriate for core features maintained in the workspace.

Benefits:

- simple data flow
- easy tests
- no IPC overhead

Costs:

- provider bugs run in the host process
- not suitable as a stable third-party ABI

## 2. Subprocess JSON-RPC Plugins

Subprocess plugins would communicate over stdin/stdout using JSON messages.
This is the likely first external plugin model.

Benefits:

- language-agnostic
- plugin crashes do not directly crash the launcher
- easier to reason about process boundaries
- protocol messages can reuse the public API shapes

Costs:

- startup and IPC overhead
- protocol versioning must be handled carefully
- cancellation, timeouts, and permissions need explicit design

## 3. WASM Plugins

WASM may provide a portable sandboxed model later, but it requires careful host
API design and runtime integration.

## 4. Dynamic Libraries

Dynamic libraries can be fast, but they are not a good first external model for
freepalette.

Risks:

- Rust ABI stability issues
- plugin crashes can crash the host
- harder sandboxing
- harder compatibility story

## Recommendation

Use built-in Rust providers for MVP features. When external plugins are added,
start with subprocess JSON-RPC/stdin-stdout plugins. Do not implement plugin
execution until timeout behavior, permissions, logging, and failure isolation
are documented.

The current daemon crate is a local service-state layer used by the CLI and UI.
It should not become an external plugin host until the subprocess protocol,
permission model, and logging rules are written down first.

## Current Status

Only built-in providers run today. No external plugin discovery, loading,
execution, marketplace, signing, or update mechanism exists yet.
