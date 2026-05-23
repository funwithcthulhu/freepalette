# Windows Daily Use

This page describes the current Windows path for running FreePalette as a local
desktop utility. It is not an installer or packaging guide.

## Current State

On Windows, `freepalette-ui` can:

- open a small command palette window;
- search built-in providers;
- use a configured global hotkey to show and focus the palette;
- hide to the tray when Escape or the window close button is used;
- show, hide, reload config, and quit from the tray menu;
- enable or disable launch at sign-in by writing a per-user Startup folder
  shortcut to the current `freepalette-ui` executable.

The UI still runs from a Cargo-built executable. There is no packaged Windows
installer yet.

## Build

From the repository root:

```powershell
cargo build -p freepalette-ui
```

For a release-style local binary:

```powershell
cargo build -p freepalette-ui --release
```

## Run

```powershell
cargo run -p freepalette-ui
```

Or run the built executable:

```powershell
target\release\freepalette-ui.exe
```

## Hotkey

Hotkeys are disabled by default. Add this to your config to try the current
Windows hotkey path:

```toml
[hotkey]
enabled = true
key = "Space"
ctrl = true
alt = true
shift = false
meta = false
```

The default enabled shape above registers `Ctrl+Alt+Space`. When the UI process
is running, pressing the hotkey shows and focuses that same process.

Supported keys are documented in [HOTKEYS.md](HOTKEYS.md).

## Tray

When tray setup succeeds, the UI creates a tray icon with a palette-and-brush
mark. The tray menu currently has these actions:

- show freepalette;
- hide freepalette;
- reload config;
- enable launch at sign-in;
- disable launch at sign-in;
- quit freepalette.

Escape hides the palette while the tray or hotkey lifecycle is active. Use
`Quit freepalette` from the tray menu to exit the process.

The checked-in `.ico` asset is not embedded into the Windows executable yet.
The current window and tray icons are set at runtime.

## Launch At Sign-In

The tray menu can create or remove a per-user Startup folder shortcut pointing
to the current `freepalette-ui` executable.

This is intentionally simple. If you enable launch at sign-in from a debug build
and later want to use a release build, disable it, run the release executable,
and enable it again from that process.

## Shell Commands

Search can display shell command results for queries beginning with `>`, but the
UI does not run shell commands. Use the CLI with `--allow-shell` when you intend
to execute a shell action.

## Current Limits

- No installer.
- No embedded Windows executable icon yet.
- No background IPC daemon.
- No macOS or Linux tray/autostart path.
- No system clipboard capture.
- No external plugin execution.
