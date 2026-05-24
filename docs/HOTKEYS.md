# Global Hotkeys

FreePalette can validate one configured global hotkey and can register it in a
foreground Windows diagnostic process.

The current Tauri UI does not register a global hotkey. The daemon crate has a
foreground diagnostic listener, but that path only logs presses.

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

The current Tauri UI does not wire global hotkey registration into the running
window. Escape closes the current window. Reintroducing a UI-owned hotkey should
also define how the window is shown, hidden, and shut down.

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
  `freepalette-daemon run`; UI registration is not wired into the current Tauri
  shell.
- macOS: not implemented.
- Linux: not implemented.

## Safety Notes

Hotkey code must not log arbitrary key presses. It logs only the configured
binding's registration state and when that specific binding is pressed.
