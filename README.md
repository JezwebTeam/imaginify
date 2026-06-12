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

## Installation status

Prebuilt downloads are not published yet. The GitHub Releases page will be used later for install-ready files:

https://github.com/JezwebTeam/imaginify/releases

For now, use one of these options:

- Run the app from source with the scripts below.
- Build your own Windows `.exe` on Windows.
- Build your own macOS `.app` and `.dmg` on macOS.

Built files are not committed to this repo because `dist/` is ignored. When release packages are ready, attach the `.exe` and `.dmg` files to a GitHub Release.

### Windows users

Current options:

1. To run from source, install Python 3.10+ and double-click `run.bat`.
2. To create a standalone app, double-click `build_windows.bat`.
3. After the build finishes, use `dist\Imaginify.exe`.
4. If Windows SmartScreen warns that the app is from an unknown publisher, click **More info** and then **Run anyway**.
5. Users do not need Python installed when running the built `.exe`.

### macOS users

Current options:

1. To run from source, install Python 3.10+ and run `bash run_macos.sh`.
2. To create a standalone Mac package, run `bash build_macos.sh` on a Mac.
3. After the build finishes, use `dist/Imaginify.app` or the generated DMG in `dist/`.
4. To install from the DMG, open it and drag `Imaginify.app` into Applications.
5. On first launch, right-click `Imaginify.app`, choose **Open**, then confirm **Open**. This is needed because the app is not notarized yet.

## How to use Imaginify

1. Add images.
   - Click **Add files** to choose individual images.
   - Click **Add folder** to import every supported image in a folder and its sub-folders.

2. Choose which images to process.
   - Tick or untick the checkbox beside each file.
   - Use **Select all** or **Select none** for batch control.
   - Click a filename to preview that image.

3. Optional: crop the selected preview image.
   - Drag across the preview to draw a crop region.
   - Click **Clear crop** to remove it.
   - The same crop area is applied to the selected batch when processing.

4. Optional: resize the images.
   - Choose a preset such as **1920w (HD)**, **Instagram 1080**, or **1080p HD**.
   - Or type a custom **Width** and/or **Height**.
   - Leave width and height blank to keep the original image size.
   - Keep **Keep aspect ratio** enabled unless you intentionally want to stretch the image.

5. Choose quality and output format.
   - Use the **Quality** slider for JPG and WEBP exports.
   - Choose **Keep original**, **JPG**, **PNG**, or **WEBP** from the format menu.
   - PNG compression is lossless, so the quality slider does not change PNG output.

6. Choose where files are saved.
   - Use **Output folder** to pick the destination.
   - Set a filename **Suffix** such as `_optimised` so the originals are not overwritten.

7. Process the selected images.
   - Click **Process selected**.
   - Watch the progress bar and status text.
   - When complete, Imaginify shows how many files succeeded and how much space was saved.

## How to use this repo

Use the repo if you want to run Imaginify from source, change the code, or build release packages.

### Clone the repo

```bash
git clone https://github.com/JezwebTeam/imaginify.git
cd imaginify
```

### Run from source

Requires Python 3.10+ (https://www.python.org/downloads/).

**Windows** - double-click `run.bat`. First run installs the required Python packages automatically.

**Mac / Linux** - in Terminal:
```bash
bash run_macos.sh
```

The launcher creates a local `.venv`, installs dependencies there, and starts the app.

### Build release packages

Each platform must be built on that platform - PyInstaller can't cross-compile.

#### Windows .exe

Double-click `build_windows.bat` on a Windows machine. Produces `dist\Imaginify.exe` (~40 MB, single file, no Python required to run).

#### macOS .app + .dmg

```bash
bash build_macos.sh
```

Produces `dist/Imaginify.app` and an architecture-labelled DMG such as `dist/Imaginify-arm64.dmg`, `dist/Imaginify-x86_64.dmg`, or `dist/Imaginify-universal2.dmg`. The build script uses a local `.venv`, bundles CustomTkinter's macOS assets, creates a proper `.icns` icon, applies an ad-hoc signature when `codesign` is available, and packages the app into a drag-to-Applications DMG.

By default, the macOS build targets the current Python architecture, usually `arm64` on Apple Silicon or `x86_64` on Intel Macs. To build another architecture, set `IMAGINIFY_TARGET_ARCH`:

```bash
IMAGINIFY_TARGET_ARCH=arm64 bash build_macos.sh
IMAGINIFY_TARGET_ARCH=x86_64 bash build_macos.sh
IMAGINIFY_TARGET_ARCH=universal2 bash build_macos.sh
```

Universal builds require a universal2 Python/Tk/Pillow stack. If that is not available, publish separate `arm64` and `x86_64` DMGs.

Drag into `/Applications`. First launch: right-click -> Open (because the app isn't notarized yet).

### Generated files

Build outputs and local environments are ignored by Git:

- `build/`
- `dist/`
- `.venv/`
- `icon.ico`
- `icon.png`
- `icon.icns`
- `icon.iconset/`

Commit source changes, scripts, and documentation. Attach built `.exe` and `.dmg` files to a GitHub Release rather than committing them to the repo.

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
| `build_macos.sh` | Build `dist/Imaginify.app` and an architecture-labelled macOS DMG |
| `push_to_github.bat` | Safely commit and push project files to `JezwebTeam/imaginify` |
| `requirements.txt` | Python dependencies |
| `requirements-build.txt` | Build-only Python dependencies |
| `.gitattributes` | Keeps shell scripts LF and Windows batch files CRLF |
| `.gitignore` | Keeps generated build outputs and local environments out of Git |
| `version.json` | App version metadata |

---

Made with care by [JezPress](https://jezpress.com/).
