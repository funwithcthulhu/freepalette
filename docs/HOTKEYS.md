# Global Hotkeys

FreePalette can validate one configured global hotkey. On Windows, the Tauri UI
can register that binding at startup, and the daemon crate can register it in a
foreground diagnostic process.

## Config Shape

```toml
[hotkey]
enabled = false
key = "Space"
ctrl = true
alt = true
shift = false
meta = false
```

The default binding is `Ctrl+Alt+Space`, but it is disabled by default.

Supported keys are intentionally narrow for now:

- `Space`
- one ASCII letter or digit
- function keys `F1` through `F24`

At least one modifier is required. FreePalette should avoid broad keyboard
capture and should only respond to a specific launcher binding.

## Tauri UI Path

On Windows, the Tauri UI reads the configured binding at startup. If the hotkey
is enabled and registration succeeds, pressing the binding shows, unminimizes,
and focuses the palette window.

When the UI has an active hotkey or tray lifecycle, Escape and the window close
button hide the palette instead of exiting the process. Use the tray menu's
Quit item to exit the background UI process.

## Windows Daemon Diagnostic Path

The daemon path uses the `global-hotkey` crate and a `tao` event loop.

Run it with:

```powershell
cargo run -p freepalette-daemon -- run
```

With the default config, this exits because the hotkey is disabled. When the
hotkey is enabled on Windows, the process registers the binding and stays in the
foreground. Pressing the hotkey writes a status line and trace event. This path
does not open or focus the UI.

## Platform Limits

- Windows: foreground diagnostic registration exists through
  `freepalette-daemon run`; the Tauri UI can also register the configured
  binding at startup.
- macOS: not implemented.
- Linux: not implemented.

## Safety Notes

Hotkey code must not log arbitrary key presses. It logs only the configured
binding's registration state and when that specific binding is pressed.
