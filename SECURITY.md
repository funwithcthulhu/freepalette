# Security Policy

Security reports should be sent privately to the project maintainers. If no
private contact has been published yet, open a minimal public issue that says
you need a private security contact, without including exploit details.

## Sensitive Areas

The following areas are security-sensitive:

- shell command execution
- app launching
- future plugin execution
- clipboard history and clipboard writes
- global hotkey registration
- config loading and file watching

Shell commands must never execute automatically from a search query. Execution
requires explicit user action.

The shared daemon state enforces a shell execution policy. The CLI requires
`--allow-shell` before executing a shell action. The minimal UI asks for a
deliberate confirmation before it passes a shell action to the daemon with shell
execution allowed.

The current `freepalette-daemon` crate is local service state, not an IPC
server. Future IPC, UI activation from hotkeys, and plugin execution work should
treat message boundaries and permissions as part of the security model, not as
UI details.

Clipboard capture and persistence are not implemented. The daemon has an
explicit in-memory clipboard buffer for future capture work, and capture is off
by default. The clipboard security model is documented in
[docs/CLIPBOARD_SECURITY.md](docs/CLIPBOARD_SECURITY.md) and must be updated
before persistent clipboard history is added.

Global hotkey registration exists for the Windows foreground daemon path. It
registers only the configured launcher binding and must not log arbitrary key
presses. The current hotkey action does not open or focus the UI yet.

## Telemetry And Secrets

freepalette must not collect telemetry. It must not collect secrets. Logs should
avoid recording sensitive clipboard contents, command output, tokens, or private
paths unless the user explicitly requests diagnostic output.

## Plugins

Third-party plugin execution is not implemented yet. It must be designed
carefully before implementation. Subprocess plugins are currently the preferred
first external model because process isolation and language-agnostic protocols
are easier to reason about than in-process dynamic loading.
