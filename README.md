# Imaginify

A modern dark-mode desktop app for Windows and macOS to resize, crop, compress and convert images - one at a time or in batches.

Made by **[JezPress](https://jezpress.com/)**.

## Features

- **Drag and drop** images or folders straight from Finder / Explorer onto the window, or use **Add files** / **Add folder** (sub-folders included).
- Per-file **checkboxes** with **Select all / Select none** - process only the images you want.
- **Quick resize presets**: 1920w, 1280w, 800w, Instagram 1080, Facebook 2048w, 1080p HD - or a custom width and height with an aspect-ratio lock.
- **Never enlarges** small images unless you switch that off.
- **Crop** each image on its own preview, freehand or locked to 1:1, 4:5, 4:3 or 16:9, and copy a crop to every ticked image in one click.
- **Format conversion**: keep the original format, JPG, PNG or WebP. Quality slider for JPG and WebP (100 on WebP = lossless).
- **Never overwrites anything.** Outputs go to your chosen folder with a suffix (default `_optimised`); if a name is taken, Imaginify saves `photo_optimised (2).jpg` instead of replacing the file - including your originals.
- **Keeps the original** when re-saving would only make a file bigger.
- **Correct colours**: iPhone (Display P3) and Adobe RGB photos are converted to standard sRGB, so they don't look washed out on the web. Photos are turned upright using their camera orientation.
- **Animated GIF, WebP and PNG** keep their animation when saved as GIF or WebP.
- **Fast batches**: several images at once, a **Cancel** button, and a report at the end listing anything that failed, with an **Open output folder** button.
- Remembers your settings between launches.

Supported files: JPG, PNG, WebP, GIF, BMP, TIFF.

## Download and install

Get the latest version from the **[Releases page](https://github.com/JezwebTeam/imaginify/releases)**.

### Windows 10 / 11 (64-bit)

- **`Imaginify-Setup-<version>.exe`** (recommended) - installer wizard with a Start Menu entry, optional Desktop icon and an uninstaller. It upgrades an existing Imaginify in place, and installs Microsoft's WebView2 runtime if the PC doesn't have it yet.
- **`Imaginify-<version>-windows-portable.exe`** - the app on its own, no installation. Needs the WebView2 runtime, which is built into Windows 11 and up-to-date Windows 10.

The app isn't code-signed yet, so Windows SmartScreen warns on first run: click **More info**, then **Run anyway**.

### macOS 11 or later

- Apple Silicon (M1 and later): **`Imaginify-<version>-macos-arm64.dmg`**
- Intel Macs: **`Imaginify-<version>-macos-x86_64.dmg`**

Open the DMG and drag Imaginify into Applications (this replaces an older version). The app isn't notarised yet, so on first launch right-click Imaginify, choose **Open**, then **Open** again.

## How to use Imaginify

1. **Add images.** Drag images or folders onto the window, or click **Add files** / **Add folder**.
2. **Choose which images to process.** Tick or untick each file, or use **Select all** / **Select none**. Click a file name (or use the arrow keys) to preview it.
3. **Optional: crop.** Drag across the preview. Pick a **Shape** first to lock the proportions. Each image keeps its own crop; **Use crop for all ticked** copies the current one to every ticked image (a locked shape stays that shape on every image). **Clear crop** removes it.
4. **Optional: resize.** Click a preset or type a **Width** and/or **Height**. Leave both blank to keep the size. Keep **Keep aspect ratio** on unless you want to stretch the image.
5. **Choose quality and format.** The **Quality** slider affects JPG and WebP; PNG is always lossless.
6. **Choose where files go.** Pick an **Output folder** and a **Suffix**.
7. **Click Process selected.** Watch the progress bar; when it finishes, Imaginify shows how much space was saved and lists any files it could not process.

## Working on Imaginify

Imaginify 2 is a [Tauri 2](https://tauri.app/) app: the image work is Rust, and the window is plain HTML, CSS and JavaScript (no framework, no bundler).

| Folder / file | What it is |
|---|---|
| `core/` | `imaginify-core`: the image pipeline - decode, crop, resize, encode, safe output naming. No UI code; covered by the tests in `core/tests/`. |
| `src-tauri/` | The desktop app: Tauri commands that call `core`, window config (`tauri.conf.json`), permissions, icons. |
| `ui/` | The window: `index.html`, `style.css`, `main.js`. |
| `installer_windows.iss`, `build_installer_windows.bat` | Windows installer (Inno Setup) and the script that builds the app and the installer. |
| `rust-toolchain.toml` | Pins the Rust version, locally and in CI. |
| `version.json`, `CHANGELOG.md` | Version metadata; the newest changelog section becomes the GitHub release notes. |
| `.github/workflows/` | `lint.yml` (checks every push) and `release.yml` (builds every platform; publishes on a `vX.Y.Z` tag). |
| `CLAUDE.md`, `.claude/` | Claude Code project notes, review agents and the `/release` skill. |

### Set up (once)

1. Install Rust: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh` (Windows: run `rustup-init.exe` from https://rustup.rs).
2. Install Node.js 20 or later.
3. macOS: `xcode-select --install`. Windows: Visual Studio Build Tools with "Desktop development with C++".
4. In the repo folder: `npm install`.

### Everyday commands

```bash
npm run dev        # run the app (rebuilds on change)
npm test           # image pipeline tests
cargo clippy --workspace --all-targets   # lint
cargo fmt --all                          # format
```

### Build packages

Each platform builds on itself. Release builds come from `release.yml`, never from a local build.

- **macOS:** `npm run build:mac` -> `target/release/bundle/macos/Imaginify.app` and a DMG in `target/release/bundle/dmg/`.
- **Windows:** double-click `build_installer_windows.bat` -> `dist\Imaginify.exe` and `dist\Imaginify-Setup-<version>.exe`. Needs [Inno Setup 6](https://jrsoftware.org/isdl.php) (`winget install --id JRSoftware.InnoSetup`); [NASM](https://www.nasm.us/) is optional and makes JPEG encoding faster.

## How the compression works

- **JPG** - mozjpeg: progressive, optimised Huffman tables, full-resolution colour (4:4:4) at quality 90 and above. Transparency is flattened onto white.
- **WebP** - libwebp at its slowest, best-compressing setting; quality 100 is lossless.
- **PNG** - oxipng: lossless, tries filter and compression combinations and reduces colour type and bit depth where that loses nothing.
- **Resize** - Lanczos3, the sharpest common filter for downscaling photos.
- Outputs carry no EXIF or other metadata, and are in sRGB.

---

Made with care by [JezPress](https://jezpress.com/).
