# Windows Daily Use

This page describes the current Windows path for running FreePalette as a local
desktop utility. It is not an installer or packaging guide.

## Current State

On Windows, `freepalette-ui` can:

- open a small command palette window;
- search built-in providers;
- move selection with the keyboard;
- execute selected non-shell actions;
- block shell actions from the UI;
- close the window with Escape.

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

The current Tauri UI does not register a global hotkey. Hotkey config validation
and the foreground daemon diagnostic path are documented in
[HOTKEYS.md](HOTKEYS.md).

## Tray

The current Tauri UI does not create a tray icon. The repository still has
Windows tray helper code from the earlier native UI path, but it is not wired
into the Tauri window lifecycle.

## Launch At Sign-In

The current Tauri UI does not expose launch-at-sign-in controls. The repository
has a Windows Startup folder helper, but the Tauri shell does not call it yet.

## Shell Commands

Search can display shell command results for queries beginning with `>`, but the
UI does not run shell commands. Use the CLI with `--allow-shell` when you intend
to execute a shell action.

## Current Limits

- No installer.
- No global hotkey in the current Tauri UI.
- No tray icon in the current Tauri UI.
- No launch-at-sign-in control in the current Tauri UI.
- No background IPC daemon.
- No macOS or Linux tray/autostart path.
- No system clipboard capture.
- No external plugin execution.
