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

- Escape closes the window.
- Restarting the UI opens a fresh window.

Current Tauri limits:

- No global hotkey is wired into the UI.
- No tray icon is wired into the UI.
- No launch-at-sign-in control is wired into the UI.

## Notes

This checklist does not replace automated tests. Add focused tests when a
behavior can be covered without depending on a real desktop session.
