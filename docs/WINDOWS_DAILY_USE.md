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
- register the configured global hotkey when enabled;
- create a tray icon;
- hide and show the palette from the tray or hotkey;
- enable or disable launch at sign-in from the tray menu.

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

The hotkey is disabled by default. Enable it in the config:

```toml
[hotkey]
enabled = true
key = "Space"
ctrl = true
alt = true
```

The Tauri UI reads this config at startup. If registration succeeds, pressing
the binding shows and focuses the palette. The foreground daemon diagnostic
path is still available for testing the binding without the UI; see
[HOTKEYS.md](HOTKEYS.md).

## Tray

The Tauri UI creates a Windows tray icon when tray creation succeeds. The tray
menu currently exposes:

- Show freepalette
- Hide freepalette
- Reload config
- Enable launch at sign-in
- Disable launch at sign-in
- Quit freepalette

## Launch At Sign-In

Launch at sign-in is controlled from the tray menu. The current implementation
creates or removes a per-user Startup folder shortcut for the running
`freepalette-ui` executable. It is not a packaged installer feature.

## Shell Commands

Search can display shell command results for queries beginning with `>`, but the
UI does not run shell commands. Use the CLI with `--allow-shell` when you intend
to execute a shell action.

## Current Limits

- No installer.
- No shell-confirmation UI.
- No background IPC daemon.
- No macOS or Linux tray/autostart path.
- No system clipboard capture.
- No external plugin execution.
