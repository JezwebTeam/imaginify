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

## Download and install

Download the latest Windows or macOS package from the GitHub Releases page:

https://github.com/JezwebTeam/imaginify/releases

### Windows users

1. Download `Imaginify.exe`.
2. Move it somewhere convenient, such as your Desktop or Downloads folder.
3. Double-click `Imaginify.exe` to open the app.
4. If Windows SmartScreen warns that the app is from an unknown publisher, click **More info** and then **Run anyway**.
5. No separate Python install is needed when using the `.exe`.

### macOS users

1. Download `Imaginify.dmg`.
2. Double-click the DMG file to open it.
3. Drag `Imaginify.app` into the **Applications** shortcut.
4. Eject the DMG.
5. Open the app from Applications.
6. On first launch, right-click `Imaginify.app`, choose **Open**, then confirm **Open**. This is needed because the app is not notarized yet.

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

Produces `dist/Imaginify.app` and `dist/Imaginify.dmg`. The build script uses a local `.venv`, bundles CustomTkinter's macOS assets, creates a proper `.icns` icon, applies an ad-hoc signature when `codesign` is available, and packages the app into a drag-to-Applications DMG.

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
| `build_macos.sh` | Build `dist/Imaginify.app` and `dist/Imaginify.dmg` |
| `requirements.txt` | Python dependencies |

---

Made with care by [JezPress](https://jezpress.com/).
