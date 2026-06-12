# Imaginify

A modern dark-mode desktop app for Windows and macOS to resize, crop, compress and convert images - in single or batch mode.

Made by **[JezPress](https://jezpress.com/)**.

## Features

- Add files or whole folders (sub-folders included).
- Per-file **checkboxes** with **Select all / Select none** - process only the images you want.
- **Quick resize presets**: 1920w, 1280w, 800w, Instagram 1080, Facebook 2048w, 1080p HD.
- Custom **Width** / **Height** with optional aspect-ratio lock.
- **Quality slider** with live value (1-100).
- Interactive **crop** - drag on the preview to set a region.
- **Format conversion**: keep original, JPG, PNG, or WEBP.
- Output folder + filename suffix (default `_optimised`).
- Live progress bar and "saved X MB" summary on completion.

## Run from source

Requires Python 3.10+ (https://www.python.org/downloads/).

**Windows** - double-click `run.bat`. First run installs Pillow + CustomTkinter automatically.

**Mac / Linux** - in Terminal:
```bash
bash run_macos.sh
```

The launcher creates a local `.venv`, installs dependencies there, and starts the app.

## Build a standalone package

Each platform must be built on that platform - PyInstaller can't cross-compile.

### Windows .exe

Double-click `build_windows.bat` on a Windows machine. Produces `dist\Imaginify.exe` (~40 MB, single file, no Python required to run).

### macOS .app + .dmg

```bash
bash build_macos.sh
```

Produces `dist/Imaginify.app` and `dist/Imaginify.dmg`. The build script uses a local `.venv`, bundles CustomTkinter's macOS assets, creates a proper `.icns` icon, applies an ad-hoc signature when `codesign` is available, and packages the app into a drag-to-Applications DMG.

Drag into `/Applications`. First launch: right-click -> Open (because the app isn't notarized yet).

## Best-quality compression notes

- **JPG** - `optimize` + `progressive` + 4:4:4 chroma subsampling at quality >= 90.
- **WebP** - `method=6` (slowest, best compression). Quality 100 = lossless.
- **PNG** - `optimize=True`, `compress_level=9` (lossless; quality slider doesn't affect PNG).
- Resize uses the **LANCZOS** filter (best for downscaling photos).
- EXIF orientation auto-corrected so portraits stay upright.

## Files in this repo

| File | Purpose |
|------|---------|
| `imaginify.py` | The app |
| `make_icon.py` | Generates `icon.ico` and `icon.png` |
| `run.bat` | Launch from source on Windows |
| `run_macos.sh` | Launch from source on macOS/Linux |
| `build_windows.bat` | Build `dist/Imaginify.exe` |
| `build_macos.sh` | Build `dist/Imaginify.app` and `dist/Imaginify.dmg` |
| `requirements.txt` | Python dependencies |

---

Made with care by [JezPress](https://jezpress.com/).
