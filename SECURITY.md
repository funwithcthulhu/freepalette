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
- config loading and file watching

Shell commands must never execute automatically from a search query. Execution
requires explicit user action.

## Telemetry And Secrets

freepalette must not collect telemetry. It must not collect secrets. Logs should
avoid recording sensitive clipboard contents, command output, tokens, or private
paths unless the user explicitly requests diagnostic output.

## Plugins

Third-party plugin execution is not implemented yet. It must be designed
carefully before implementation. Subprocess plugins are currently the preferred
first external model because process isolation and language-agnostic protocols
are easier to reason about than in-process dynamic loading.
