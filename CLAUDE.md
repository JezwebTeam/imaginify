# CLAUDE.md - Imaginify

Dark-mode desktop app (Windows + macOS) that resizes, crops, compresses and converts images, single or batch. Made by JezPress. Not a WordPress plugin: the JezPress plugin release rules do not apply here.

> Dev tooling lives in `.claude/` (see `.claude/README.md`). Audit gates: `.claude/audit-guide.md` (load on demand). Releases: the **`/release` skill**. Release history: `.claude/LEDGER.md`.

| | |
|---|---|
| App | `imaginify.py` (single file: Pillow pipeline + CustomTkinter UI) |
| Version | 1.0.1 |
| Python | 3.10+ (README and both build scripts enforce it). The macOS system `python3` is 3.9 without Pillow - use `.venv/bin/python` for anything that imports the app |
| Dependencies | `requirements.txt` (Pillow, customtkinter, darkdetect, tkinterdnd2); build-only `requirements-build.txt` (PyInstaller, pinned) |
| Packaging | PyInstaller: `build_windows.bat` -> `dist/Imaginify.exe`; `build_macos.sh` -> `dist/Imaginify.app` + `Imaginify-<arch>.dmg`. Installers: `installer_windows.iss` (Inno Setup) and `build_installer_macos.sh` (pkgbuild + productbuild). Each platform builds only on itself |
| Bundle id | `com.jezweb.imaginify`; Inno Setup `AppId` `{1A9A715E-...}` - never change either, upgrades depend on them |
| Repo | https://github.com/JezwebTeam/imaginify |

## Version spots (all move together)

`imaginify.py` `__version__` · `version.json` `"version"` · `installer_windows.iss` `MyAppVersion` · `build_installer_macos.sh` `APP_VERSION` · `installer_macos/distribution.xml` `pkg-ref version` · newest `CHANGELOG.md` section. `python3 .claude/scripts/check_versions.py` checks them; CI fails when they disagree.

## Rules that are easy to break

- **Never touch a user's source image.** Outputs go to the chosen folder with a suffix; nothing is written next to the source unless the user chose that folder.
- **Tk only on the main thread.** `_process_all` runs on a worker thread and must reach widgets through `root.after` (`_set_status`, `_set_progress`).
- **New imports reach the frozen build** only via `--collect-all` / `--hidden-import` in BOTH `build_windows.bat` and `build_macos.sh`.
- **Every input mode against every output format.** JPEG has no alpha (flatten to white), PNG has no CMYK, Pillow's TIFF format name is `TIFF` not `TIF`.
- **Workflows:** never `${{ }}` inside a `run:` block - pass values through `env:`.

## Run and build

```bash
bash run_macos.sh      # creates .venv, installs deps, launches the app
bash build_macos.sh    # dist/Imaginify.app + DMG
```

Built files (`build/`, `dist/`, icons, `.venv/`) are git-ignored; release builds come from `release.yml`, never from a local `dist/`.
