# Imaginify

A modern dark-mode desktop app for Windows and macOS to resize, crop, compress and convert images - in single or batch mode.

Made by **[JezPress](https://jezpress.com/)**.

![Imaginify icon](icon.png)

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
- **Auto update check** against `https://jezpress.com/imaginify/version.json` - silent on startup, plus a manual "Check for updates" button in the header.

## Run from source

Requires Python 3.10+ (https://www.python.org/downloads/).

**Windows** - double-click `run.bat`. First run installs Pillow + CustomTkinter automatically.

**Mac / Linux** - in Terminal:
```bash
pip3 install -r requirements.txt
python3 imaginify.py
```

## Build a standalone package

Each platform must be built on that platform - PyInstaller can't cross-compile.

### Windows .exe

Double-click `build_windows.bat` on a Windows machine. Produces `dist\Imaginify.exe` (~40 MB, single file, no Python required to run).

### macOS .app

```bash
bash build_macos.sh
```

Produces `dist/Imaginify.app`. Drag into `/Applications`. First launch: right-click -> Open (because the app isn't code-signed yet).

## Update server

The app checks `UPDATE_CHECK_URL = "https://jezpress.com/imaginify/version.json"` on startup (silently) and via the "Check for updates" button.

Upload a JSON file at that URL with this shape to push updates to users:

```json
{
  "version": "1.1.0",
  "url": "https://jezpress.com/imaginify",
  "notes": "Bug fixes and a new preset"
}
```

When the version is newer than the user's installed `__version__`, the app shows a dialog with the release notes and offers to open the URL in their browser. There's a template `version.json` in the repo you can copy onto the JezPress server.

To release a new version: bump `__version__` in `imaginify.py`, rebuild the .exe/.app, upload them to your download page, and update `version.json` on the server.

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
| `build_windows.bat` | Build `dist/Imaginify.exe` |
| `build_macos.sh` | Build `dist/Imaginify.app` |
| `version.json` | Template for the update server |
| `requirements.txt` | Python dependencies |

---

Made with care by [JezPress](https://jezpress.com/).
