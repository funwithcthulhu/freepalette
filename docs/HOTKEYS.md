# Global Hotkeys

FreePalette can register one configured global hotkey on Windows.

The UI process owns the useful path today: when the hotkey is enabled,
`freepalette-ui` registers it and uses it to show and focus the local palette
process. The daemon crate also has a foreground diagnostic listener, but that
path only logs presses.

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

## Windows UI Path

The UI path uses the `global-hotkey` crate from the egui process. This keeps raw
Win32 calls out of the repo while the workspace keeps `unsafe_code` forbidden.

Run it with an enabled hotkey config:

```powershell
cargo run -p freepalette-ui
```

Pressing the configured binding shows and focuses that same UI process. Escape
hides the palette while the hotkey bridge or tray lifecycle is active.

On Windows, the UI also creates a tray icon when tray setup succeeds. The tray
can show the palette even when the hotkey is disabled or unavailable.

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

- Windows: UI registration exists through `freepalette-ui`; foreground
  diagnostic registration exists through `freepalette-daemon run`.
- macOS: not implemented.
- Linux: not implemented.

## Safety Notes

Hotkey code must not log arbitrary key presses. It logs only the configured
binding's registration state and when that specific binding is pressed.
