# Icons

The current icon is a small paint palette with a brush.

Files:

- `freepalette.svg`: editable source asset.
- `freepalette.ico`: Windows icon container with 16, 32, 48, and 256 px images.
- `freepalette-16.png`, `freepalette-32.png`, `freepalette-48.png`,
  `freepalette-256.png`: rendered raster sizes for docs, packaging, and manual
  inspection.

The egui window icon and Windows tray icon are generated from matching Rust
pixel drawing code in `crates/freepalette-ui/src/icon.rs`. A future packaging
pass can embed `freepalette.ico` as the Windows executable icon.
