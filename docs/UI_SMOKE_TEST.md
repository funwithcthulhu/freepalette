# UI Smoke Test Checklist

This is a manual checklist for the current Tauri desktop UI. It is meant for
maintainers before a release or after changes to the palette window.

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

- On Windows, the tray icon appears when tray creation succeeds.
- Escape hides the window when the tray or hotkey lifecycle is active.
- Left-clicking the tray icon shows the palette.
- The tray menu can show, hide, reload config, and quit the UI.
- The tray menu enables or disables launch at sign-in without panicking.
- With an enabled Windows hotkey, pressing the configured binding shows and
  focuses the palette.

Current Tauri limits:

- No installer is implemented.
- No shell-confirmation UI is implemented.
- No macOS or Linux tray/autostart path is implemented.

## Notes

This checklist does not replace automated tests. Add focused tests when a
behavior can be covered without depending on a real desktop session.
