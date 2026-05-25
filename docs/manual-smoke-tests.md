# Manual Smoke Tests

This checklist is for a Windows maintainer doing a local check before a release
or after changing CLI, app indexing, shell execution, Tauri UI, lifecycle, or
installer behavior.

Automated tests cover the core behavior where possible. These checks cover the
desktop surfaces that still need a real Windows session.

## Setup

From the repository root:

```powershell
git status --short --branch
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

Expected behavior:

- `git status` shows the intended branch and no unrelated local changes.
- `fmt`, `clippy`, and `test` complete without errors.

## Config Path

```powershell
cargo run -p freepalette-cli -- config-path
```

Expected behavior:

- Prints one line.
- On Windows, the line is a platform config path ending in `freepalette.toml`.
- If a platform config directory cannot be resolved, the line says
  `default config path is unavailable on this platform`.

## Windows Start Menu Indexing

```powershell
cargo run -p freepalette-cli -- apps list
```

Expected behavior:

- Prints a `status:` line.
- Prints an `apps:` count.
- Real Start Menu entries, when found, show `source: windows-start-menu`.
- Known built-in entries, when present, show `source: known`.
- Fallback entries, when used, show `source: fallback`.
- The command does not launch any app.

## Calculator Query

```powershell
cargo run -p freepalette-cli -- search "calc 2+2"
```

Expected behavior:

- Prints a calculator result containing `2+2 = 4`.
- Shows an action that copies or exposes the calculated text.
- Does not open the UI.

## Shell Query Preview Without Execution

Use a marker file so the check proves search did not run the shell command:

```powershell
$marker = Join-Path $env:TEMP "freepalette-shell-preview-marker.txt"
Remove-Item -LiteralPath $marker -ErrorAction SilentlyContinue
cargo run -p freepalette-cli -- search "> echo shell-ran > `"$marker`""
Test-Path -LiteralPath $marker
```

Expected behavior:

- Search prints a shell result and an action containing `run shell command`.
- Search does not print `Running:`.
- `Test-Path` prints `False`.

## Explicit Run Without `--allow-shell`

```powershell
cargo run -p freepalette-cli -- run "> echo hello"
```

Expected behavior:

- Exits with an error.
- Does not print `Running:`.
- Error text includes `refusing to run shell command without --allow-shell`.

## Explicit Run With `--allow-shell`

```powershell
cargo run -p freepalette-cli -- run "> echo hello" --allow-shell
```

Expected behavior:

- Prints `Running: [shell] Run: echo hello`.
- Prints a shell exit message such as `shell exited with 0: hello`.
- Executes only after the explicit `run` command and `--allow-shell` flag.

## Tauri UI Startup

```powershell
cargo run -p freepalette-ui
```

Expected behavior:

- A small FreePalette window opens.
- The search input is focused.
- `calc 2+2` shows the calculator result.
- `> echo hello` shows a shell result and asks for confirmation before running.
- Escape hides or closes the window according to the active lifecycle state.

## Tray And Hotkey Behavior

Use this section only when testing on Windows with hotkey config enabled.

Example config:

```toml
[hotkey]
enabled = true
key = "Space"
ctrl = true
alt = true
shift = false
meta = false
```

Expected behavior:

- The Tauri UI creates a tray icon when tray creation succeeds.
- The tray menu can show, hide, reload config, toggle launch at sign-in, and
  quit.
- Pressing the configured hotkey shows and focuses the palette.
- No arbitrary key presses are logged.

## Installer Build And Install

Build the local unsigned NSIS installer:

```powershell
cd crates\freepalette-ui
cargo tauri build --bundles nsis --ci
```

Expected behavior:

- The command reports a bundle under `target\release\bundle\nsis`.
- The installer filename includes the current app version.
- The installer is unsigned, so Windows may warn before running it.
- Installing and launching the app opens the same Tauri UI as the Cargo-built
  executable.
- Launch at sign-in remains controlled by the tray menu, not by installer
  defaults.

## Last Manually Checked

Partial Windows check on 2026-05-25:

- `config-path` printed a Windows config path ending in `freepalette.toml`.
- `apps list` reported 131 apps, including real entries from two Windows Start
  Menu roots and the known Notepad entry.
- `search "calc 2+2"` returned `2+2 = 4`.
- Shell search preview for a marker-file command did not create the marker file.
- `run "> echo hello"` refused without `--allow-shell`.
- `run "> echo hello" --allow-shell` printed `hello` after explicit execution.
- `cargo build -p freepalette-ui` passed.
- Running `target\debug\freepalette-ui.exe` opened a `freepalette` window and
  stayed alive.
- With a temporary config enabling `Ctrl+Alt+Space`, the UI logged
  `UI global hotkey registered`.
- `cargo tauri build --bundles nsis --ci` produced
  `target\release\bundle\nsis\freepalette_0.6.0_x64-setup.exe`.

Not checked in that pass:

- Typing queries into the live Tauri window and confirming shell commands from
  the UI.
- Tray menu show, hide, reload config, launch-at-sign-in, and quit actions.
- Pressing the registered global hotkey.
- Running the unsigned installer and launching the installed app.
