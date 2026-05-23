# UI Smoke Test Checklist

This is a manual checklist for the current desktop UI. It is meant for
maintainers before a release or after changes to UI lifecycle code.

## Build Checks

Run these first:

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

## Manual Windows UI Checks

Start the UI:

```powershell
cargo run -p freepalette-ui
```

Check the palette:

- The window opens with the search input focused.
- `calc 2+2` shows a calculator result.
- `notepad` shows an app result on Windows.
- `> echo hello` shows a shell result and labels it as blocked.
- Enter on `calc 2+2` reports that the calculator result is ready to copy.
- Enter on `> echo hello` refuses shell execution.
- Arrow up/down changes the selected row without panicking.

Check lifecycle behavior:

- Escape hides the window while the tray or hotkey lifecycle is active.
- The tray menu can show the hidden window.
- The tray menu can hide the visible window.
- The tray menu can reload config and keep the process running.
- The tray menu can quit the process.

Check launch at sign-in:

- Enable launch at sign-in from the tray menu.
- The UI reports that a Startup folder shortcut was created.
- Disable launch at sign-in from the tray menu.
- The UI reports that the shortcut was removed.

Check hotkey behavior if enabled in config:

- The configured binding shows and focuses the UI process.
- The app does not log arbitrary key presses.

## Notes

This checklist does not replace automated tests. Add focused tests when a
behavior can be covered without depending on a real desktop session.
