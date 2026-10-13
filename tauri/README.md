# Imaginify 2 (Tauri + Rust prototype)

A rewrite of Imaginify as a Tauri 2 app. The image work is Rust; the window is plain HTML/CSS/JS with no framework or bundler. The Python app in the repo root is unchanged.

| Folder | What it is |
|---|---|
| `core/` | `imaginify-core`: the image pipeline (decode, crop, resize, encode, safe output naming). No UI code, fully tested. |
| `src-tauri/` | The desktop app: Tauri commands that call `core`, window config, permissions, icons. |
| `ui/` | The window: `index.html`, `style.css`, `main.js`. Talks to Rust through `window.__TAURI__`. |

## Setup (once)

1. Rust: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
2. macOS: Xcode command line tools (`xcode-select --install`). Windows: Visual Studio Build Tools with "Desktop development with C++"; WebView2 ships with Windows 10/11.
3. `npm install` in this folder (installs the pinned Tauri CLI).

## Everyday commands (from this folder)

```bash
npm run dev      # run the app (rebuilds Rust on change)
npm test         # pipeline tests (cargo test -p imaginify-core)
npm run build    # release build + installers in target/release/bundle/
cargo clippy --workspace --all-targets   # lint
cargo fmt --all                          # format
```

## How it behaves differently from 1.x

- **Never replaces a file.** Outputs are written to a hidden temp file, then renamed onto the first free name (`photo_optimised.jpg`, `photo_optimised (2).jpg`, ...) with an atomic no-replace rename. The source, earlier outputs, and other files in the same batch are all safe, and quitting mid-batch never leaves a half-written image.
- **Crop is per image**, stored as fractions of the image, so it survives window resizes. "Use crop for all ticked" copies it to other images on purpose. Optional shape lock (1:1, 4:5, 4:3, 16:9).
- **Colour:** embedded profiles (Display P3, Adobe RGB) are converted to sRGB, so iPhone photos no longer look washed out. EXIF orientation is applied.
- **Formats:** 16-bit and CMYK inputs work for every output; `.tif` keeps its spelling; animated GIFs keep their frames when the output is GIF.
- **Size:** JPEG via mozjpeg (progressive, optimised Huffman, 4:4:4 chroma at quality 90+), PNG via oxipng (lossless), WebP via libwebp (method 6; quality 100 = lossless). "Don't enlarge" and "Keep the original if re-saving makes it bigger" are on by default.
- **Faster batches:** up to 4 images at once, with a Cancel button and a report listing every failure.
- **Settings are remembered** between launches.

## Not done yet

HEIC input, AVIF output, the CI release workflow and installers for this build, code signing, a Windows test run, and keeping Inno Setup's `AppId` so 1.x installs upgrade in place.
