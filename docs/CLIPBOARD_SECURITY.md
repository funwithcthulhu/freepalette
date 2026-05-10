# Clipboard Security Model

Clipboard history is security-sensitive. Users routinely copy passwords,
tokens, private URLs, addresses, and work documents. FreePalette must treat
clipboard capture and storage as local sensitive data handling, not as a normal
search index.

## Current State

Clipboard capture and persistence are not implemented.

The current daemon owns an in-memory clipboard history buffer. It can accept
explicit entries from code paths that call into the daemon, but the application
does not watch the system clipboard, write a history file, or sync clipboard
data.

Provider registration and clipboard capture are separate concerns. Enabling the
clipboard provider today does not capture clipboard contents.

Clipboard capture is off by default:

```toml
[clipboard]
capture = false
max_entries = 50
max_entry_bytes = 4096
```

## Local Storage

Future clipboard history storage must be local-only:

- no account requirement;
- no cloud sync;
- no telemetry;
- no remote plugin access to clipboard entries;
- no clipboard contents in crash reports or diagnostics.

The first persistent implementation should use an application-owned local data
file and document its path. If encryption or OS credential storage is added, it
must be described as a separate security decision. Until then, documentation
must not imply that persisted clipboard history is encrypted.

## Retention

Clipboard retention must be bounded before persistence is implemented.

The first persistent implementation should define:

- maximum number of retained entries;
- maximum size per retained entry;
- whether duplicate entries are coalesced;
- whether entries expire by age;
- what happens when limits are exceeded.

The defaults should prefer smaller retention over completeness. Retention
limits should be testable in core code without needing a real system clipboard.

## Sensitive Content Exclusions

Automatic secret detection is not a reliable security boundary. FreePalette may
add best-effort exclusions, but docs and UI must not promise that all secrets
are detected.

Practical exclusions to consider before capture ships:

- ignore empty or whitespace-only clipboard entries;
- ignore entries above the configured size limit;
- allow users to disable clipboard history entirely;
- allow users to clear stored history;
- consider user-controlled ignore patterns or source-application exclusions
  before storing entries from password managers or terminals.

## Logging

Logs must not include clipboard contents, previews, or stable hashes of
clipboard contents. Logging counts, byte lengths, and state transitions is
acceptable when useful.

Error messages should identify the operation that failed without echoing the
clipboard entry.

## User Controls

Before persistent clipboard history ships, FreePalette should provide:

- a config switch to disable clipboard history;
- a way to clear stored history;
- documented storage location;
- documented retention defaults;
- tests for disabled, clear, and retention behavior.

The existing `[providers].clipboard` switch controls whether the clipboard
provider is registered. The `[clipboard].capture` switch controls whether
daemon code may record clipboard entries. Capture remains off by default.

## Test Guardrails

Clipboard tests should cover:

- preview truncation;
- no full clipboard contents in action messages;
- disabled provider registration;
- retention limit behavior before persistence;
- clear-history behavior before persistence;
- no logs containing clipboard entry text once logging exists for this path.
