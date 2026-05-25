# Windows Daily Use

This page describes the current Windows path for running FreePalette as a local
desktop utility. For installer commands, see
[WINDOWS_INSTALLER.md](WINDOWS_INSTALLER.md).

## Current State

On Windows, `freepalette-ui` can:

- open a small command palette window;
- search built-in providers;
- move selection with the keyboard;
- execute selected non-shell actions;
- ask for confirmation before running a selected shell action;
- register the configured global hotkey when enabled;
- create a tray icon;
- hide and show the palette from the tray or hotkey;
- enable or disable launch at sign-in from the tray menu;
- build a local unsigned NSIS installer through Tauri.

The installer path is basic packaging groundwork. It is not signed and is not
published as an official release artifact yet.

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

## Installer

The Tauri config can build a local NSIS installer:

```powershell
cd crates\freepalette-ui
cargo tauri build --bundles nsis --ci
```

The installer is written under `target\release\bundle\nsis`. It is unsigned.

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
UI asks for confirmation before running the selected shell command. Search alone
does not execute shell commands. The CLI still requires `--allow-shell` when you
intend to execute a shell action there.

## Current Limits

- No signed or published installer.
- No auto-update.
- No dedicated shell-confirmation review surface.
- No background IPC daemon.
- No macOS or Linux tray/autostart path.
- No system clipboard capture.
- No external plugin execution.
