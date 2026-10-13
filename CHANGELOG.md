# Changelog

All notable changes to Imaginify. Newest first. Each `## [X.Y.Z]` section is the
GitHub release body for that tag, so keep it short, user-facing and ASCII.

## [Unreleased]

Draft for 2.0.0 - `/release` turns this into the release section.

- Rebuilt from the ground up (Rust + Tauri): a much smaller download, and batches process several images at once.
- Never overwrites anything: if a file name is taken (including your original), the output is saved as "name (2)" instead.
- Crop is now per image, with optional 1:1, 4:5, 4:3 and 16:9 shapes, and "Use crop for all ticked" to copy one crop to a batch.
- iPhone and other wide-colour photos keep their colours (converted to sRGB), and photos are turned upright using their camera orientation.
- Animated GIF, WebP and PNG files keep their animation when saved as GIF or WebP.
- Smaller files: JPEG via mozjpeg, PNG via oxipng; small images are no longer enlarged, and a file is kept as-is when re-saving would make it bigger.
- Cancel button, a report listing any files that failed, an "Open output folder" button, and settings remembered between launches.
- Fixed: TIFF files with "Keep original", CMYK and 16-bit images, and crops drifting when the window was resized.
- macOS downloads are now DMG only; the Windows installer upgrades 1.x in place and installs the WebView2 runtime if needed.

## [1.0.1] - 2026-07-02

- Drag and drop: drop images or folders from Finder / File Explorer onto the window. Folders are scanned recursively for supported images.
- Windows installer wizard (Inno Setup) and macOS installer wizard (.pkg), alongside the portable .exe and the .dmg.

## [1.0.0] - 2026-05-15

- First release: resize, crop, compress and convert images (JPG, PNG, WEBP) in single or batch mode, with quick presets, a quality slider and per-file selection.
