# Windows Installer

FreePalette has a checked-in Tauri bundle configuration for a local Windows
NSIS installer. This is packaging groundwork, not a release pipeline.

## Current State

- The Tauri bundle target is `nsis`.
- The installer uses the checked-in palette-and-brush icons.
- The installer is unsigned.
- There is no updater.
- There is no publishing automation.
- Launch at sign-in remains controlled by the tray menu after the app is
  installed.

## Build

From the UI crate:

```powershell
cd crates\freepalette-ui
cargo tauri build --bundles nsis --ci
```

The installer is written under the workspace `target\release\bundle\nsis`
directory.

For a release executable without building an installer:

```powershell
cd crates\freepalette-ui
cargo tauri build --no-bundle --ci
```

## Local-First Behavior

The installer does not add accounts, cloud sync, telemetry, or network behavior.
The installed app uses the same local config path as the development binary.

## Signing

Windows may warn when running the installer because it is not code signed. Do
not present an installer as an official release artifact until signing and
release provenance are documented.

## Not Included Yet

- Signed installer artifacts.
- Auto-update.
- Release upload automation.
- MSI packaging.
- Per-platform installer documentation beyond Windows.
