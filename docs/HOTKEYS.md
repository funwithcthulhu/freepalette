# Global Hotkeys

FreePalette does not register a live global hotkey yet.

The current daemon crate parses and validates a small hotkey config shape and
reports whether the configured binding is ready for a future Windows message
loop. The binary still initializes state and exits, so there is no process
alive to receive hotkey messages.

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

The likely first Windows implementation is a long-running daemon with a normal
message loop and a single registered launcher hotkey. That code should open or
focus the local palette only.

Direct Windows hotkey registration is not wired in this repo yet because the
daemon exits immediately. Registering a hotkey without a message loop would
pretend the feature works when it cannot receive events.

## Platform Limits

- Windows: config validation exists; live registration waits for the daemon
  loop.
- macOS: not implemented.
- Linux: not implemented.

## Safety Notes

Hotkey code must not log arbitrary key presses. It should log only state
changes such as enabled, disabled, registered, failed, or unsupported.
