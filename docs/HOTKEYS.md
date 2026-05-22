# Global Hotkeys

FreePalette can register one configured global hotkey on Windows when the
daemon is started in foreground mode.

The current daemon crate parses and validates a small hotkey config shape. The
default daemon command still initializes state and exits. `freepalette-daemon
run` starts the foreground listener path.

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

## Windows Path

The Windows path uses the `global-hotkey` crate and a `tao` event loop. This
keeps raw Win32 calls out of the repo while the workspace keeps `unsafe_code`
forbidden.

Run it with:

```powershell
cargo run -p freepalette-daemon -- run
```

With the default config, this exits because the hotkey is disabled. When the
hotkey is enabled on Windows, the process registers the binding and stays in the
foreground. Pressing the hotkey writes a status line and trace event. Opening or
focusing the palette is not wired yet.

## Platform Limits

- Windows: foreground registration exists through `freepalette-daemon run`.
- macOS: not implemented.
- Linux: not implemented.

## Safety Notes

Hotkey code must not log arbitrary key presses. It logs only the configured
binding's registration state and when that specific binding is pressed.
