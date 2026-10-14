# CLAUDE.md - Imaginify

Dark-mode desktop app (Windows + macOS) that resizes, crops, compresses and converts images, single or batch. Made by JezPress. Not a WordPress plugin: the JezPress plugin release rules do not apply here (the Google Chat announcement does - see the `/release` skill).

> Dev tooling lives in `.claude/` (see `.claude/README.md`). Audit gates: `.claude/audit-guide.md` (load on demand). Releases: the **`/release` skill**. Release history: `.claude/LEDGER.md`.

| | |
|---|---|
| App | Tauri 2. `core/` = `imaginify-core`, the UI-free Rust image pipeline (tests in `core/tests/`); `src-tauri/` = Tauri shell (commands in `src/main.rs`, `tauri.conf.json`, `capabilities/`); `ui/` = plain HTML/CSS/JS window (no framework, no bundler) |
| Version | 1.1.0 (first Tauri release; 1.0.x was Python) |
| Toolchain | Rust pinned by `rust-toolchain.toml` (rustup at `~/.cargo/bin`, may need adding to PATH); Node 20+ for the Tauri CLI (`npm install`) |
| Key crates | image, fast_image_resize, mozjpeg, oxipng, webp (libwebp), lcms2, tempfile, rayon |
| Packaging | macOS: `npm run build:mac` -> ad-hoc-signed `.app` + DMG. Windows: `build_installer_windows.bat` -> `dist\Imaginify.exe` + Inno Setup `dist\Imaginify-Setup-<v>.exe` (installs WebView2 if missing). Each platform builds only on itself; releases come from `release.yml` |
| Bundle id | `com.jezweb.imaginify`; Inno Setup `AppId` `{1A9A715E-...}` - never change either, upgrades from 1.0.x (Python) depend on them |
| Repo | https://github.com/JezwebTeam/imaginify |

## Version spots (all move together)

`src-tauri/tauri.conf.json` · `core/Cargo.toml` · `src-tauri/Cargo.toml` · the two workspace entries in `Cargo.lock` · `package.json` · `package-lock.json` (two fields) · `installer_windows.iss` `MyAppVersion` · `version.json` · newest `CHANGELOG.md` section. `python3 .claude/scripts/check_versions.py` checks them; CI fails when they disagree. `/release` step 2 has the bump script.

## Rules that are easy to break

- **Never replace a file.** Every output goes through `core/src/output.rs` `write_unique` (temp file + atomic no-replace rename; exclusive create as fallback). Nothing else writes in the output folder.
- **Never allocate from an untrusted size.** A failed allocation aborts the app and `catch_unwind` cannot stop it. Input files are capped at `MAX_INPUT_BYTES` before any decoder runs; sizes from file headers or from the user are checked against `MAX_STILL_BYTES`, `MAX_ANIMATION_BYTES` / `MAX_FRAMES` and `MAX_OUTPUT_PIXELS` (plus the resizer's working memory) before allocating. WebP is structure-checked first, and its ICC/EXIF are read from that check, never from the WebP decoder's own metadata index.
- **Every input mode against every output format.** Decode normalises to L8/La8/Rgb8/Rgba8 in sRGB; JPEG has no alpha (flatten to white); WebP is limited to 16383 px per side. Add a test in `core/tests/pipeline.rs` for any new case.
- **Crops are fractions of the upright image**, with an optional locked `ratio` held in pixels per image (`geometry::crop_rect`; `fitted()` in `ui/main.js` mirrors it).
- **Blocking work off the async runtime.** Tauri commands wrap decoding/encoding in `spawn_blocking`; previews go through `PreviewGate` (one at a time, stale ones skipped).
- **A new `window.__TAURI__` call needs its permission** in `src-tauri/capabilities/default.json`, or it fails only at runtime.
- **File names reach the DOM via `textContent` only.**
- **Workflows:** never `${{ }}` inside a `run:` block - pass values through `env:`.

## Known limitations (accepted, do not re-raise)

**A deliberately crafted image can still crash the app** through memory exhaustion deep inside a third-party decoder (e.g. lossless-WebP Huffman tables). It cannot replace or damage any file, but it can leave a hidden `.imaginify-*.tmp` file in the output folder and the rest of the batch undone. Accepted for 1.1.0 by Abner on 2026-10-14; 1.1.1 moves decoding into a worker process so a decoder crash becomes a per-file error. Do not chase further decoder-internal allocation paths before that.

CMYK and greyscale ICC profiles are approximated (the decoder converts CMYK before the profile can be applied); the last frame of an animated WebP output gets a duration libwebp estimates; play-once GIFs are written as looping; EXIF orientation on animated WebP is ignored; multi-page TIFFs keep page 1; 16-bit animated PNGs keep only the first frame (with a note); files over 512 MB are refused; on volumes without no-replace rename or hard links, killing the app mid-write can leave one partial new file (never a replaced one); Cmd+Q on macOS skips the "still processing" prompt. Outputs carry no EXIF/metadata and are sRGB; 16-bit sources become 8-bit (deliberate).

## Run and build

```bash
npm install          # once
npm run dev          # run the app
npm test             # pipeline tests
npm run build:mac    # .app + DMG in target/release/bundle/
```

Built files (`target/`, `node_modules/`, `dist/`, `src-tauri/gen/`) are git-ignored; release builds come from `release.yml`, never from a local build.
