#!/usr/bin/env bash
# ----------------------------------------------------------------
# Imaginify - macOS build script
# Produces dist/Imaginify.app and dist/Imaginify.dmg
# Run from Terminal:  bash build_macos.sh
# ----------------------------------------------------------------
set -euo pipefail
cd "$(dirname "$0")"

echo "=== Imaginify macOS build ==="
echo

if [ "$(uname -s)" != "Darwin" ]; then
    echo "This script must be run on macOS."
    echo "PyInstaller cannot cross-compile a macOS .app from Windows or Linux."
    exit 1
fi

if ! command -v python3 >/dev/null 2>&1; then
    echo "Python 3 not found."
    echo "Install Python 3.10+ from https://www.python.org/downloads/macos/"
    echo "or run:  brew install python"
    exit 1
fi

python3 -c "import sys; raise SystemExit(0 if sys.version_info >= (3, 10) else 1)" || {
    echo "Python 3.10+ is required."
    echo "Current version: $(python3 --version)"
    exit 1
}

if [ ! -x ".venv/bin/python" ]; then
    echo "Creating local virtual environment..."
    python3 -m venv .venv
    echo
fi

PYTHON=".venv/bin/python"
PY_ARCH=$("$PYTHON" -c "import platform; print(platform.machine())")
echo "Python architecture: $PY_ARCH"
echo "Python version: $("$PYTHON" --version)"
echo

echo "Installing build dependencies..."
"$PYTHON" -m pip install --upgrade pip
"$PYTHON" -m pip install -r requirements.txt pyinstaller
echo

echo "Generating icon..."
"$PYTHON" make_icon.py
echo

# Convert icon.png -> icon.icns for crisp Mac rendering
ICON_ARGS=()
if command -v iconutil >/dev/null 2>&1 && command -v sips >/dev/null 2>&1; then
    echo "Converting icon to .icns ..."
    rm -rf icon.iconset icon.icns
    mkdir -p icon.iconset
    sips -z 16 16     icon.png --out icon.iconset/icon_16x16.png       >/dev/null
    sips -z 32 32     icon.png --out icon.iconset/icon_16x16@2x.png    >/dev/null
    sips -z 32 32     icon.png --out icon.iconset/icon_32x32.png       >/dev/null
    sips -z 64 64     icon.png --out icon.iconset/icon_32x32@2x.png    >/dev/null
    sips -z 128 128   icon.png --out icon.iconset/icon_128x128.png     >/dev/null
    sips -z 256 256   icon.png --out icon.iconset/icon_128x128@2x.png  >/dev/null
    sips -z 256 256   icon.png --out icon.iconset/icon_256x256.png     >/dev/null
    sips -z 512 512   icon.png --out icon.iconset/icon_256x256@2x.png  >/dev/null
    sips -z 512 512   icon.png --out icon.iconset/icon_512x512.png     >/dev/null
    sips -z 1024 1024 icon.png --out icon.iconset/icon_512x512@2x.png   >/dev/null
    iconutil -c icns icon.iconset
    ICON_ARGS=(--icon icon.icns)
else
    echo "iconutil/sips not available - using the default PyInstaller icon."
fi
echo

echo "Cleaning previous build..."
rm -rf build dist Imaginify.spec
echo

echo "Building Imaginify.app with PyInstaller (this can take a minute)..."
# --collect-all customtkinter is critical on macOS:
#   --collect-data alone misses theme JSON files, which makes the window render blank
# --collect-all darkdetect handles CTk's appearance-mode detection
# --target-arch matches the running Python so we don't end up with a wrong-arch binary
TARGET_ARCH_ARGS=()
if [ "$PY_ARCH" = "arm64" ] || [ "$PY_ARCH" = "x86_64" ]; then
    TARGET_ARCH_ARGS=(--target-arch "$PY_ARCH")
fi

"$PYTHON" -m PyInstaller \
    --windowed \
    --name Imaginify \
    "${ICON_ARGS[@]}" \
    --collect-all customtkinter \
    --collect-all darkdetect \
    --hidden-import darkdetect \
    --osx-bundle-identifier com.jezweb.imaginify \
    "${TARGET_ARCH_ARGS[@]}" \
    --clean \
    --noconfirm \
    imaginify.py

if command -v codesign >/dev/null 2>&1; then
    echo
    echo "Applying ad-hoc code signature..."
    codesign --force --deep --sign - dist/Imaginify.app || {
        echo "WARNING - ad-hoc code signing failed. The app was still built."
    }
fi

APP_PATH="dist/Imaginify.app"
DMG_STAGING_DIR="build/dmg"
DMG_PATH="dist/Imaginify.dmg"

if command -v hdiutil >/dev/null 2>&1; then
    echo
    echo "Creating DMG installer..."
    rm -rf "$DMG_STAGING_DIR" "$DMG_PATH"
    mkdir -p "$DMG_STAGING_DIR"
    cp -R "$APP_PATH" "$DMG_STAGING_DIR/"
    ln -s /Applications "$DMG_STAGING_DIR/Applications"
    hdiutil create \
        -volname Imaginify \
        -srcfolder "$DMG_STAGING_DIR" \
        -ov \
        -format UDZO \
        "$DMG_PATH" >/dev/null
else
    echo
    echo "WARNING - hdiutil not found, so the DMG was not created."
fi

echo
echo "=== BUILD SUCCESSFUL ==="
echo
echo "Find your app at:  $(pwd)/dist/Imaginify.app"
if [ -f "$DMG_PATH" ]; then
    echo "Find your DMG at:  $(pwd)/$DMG_PATH"
fi
echo
echo "Verifying CustomTkinter assets were bundled..."
CTK_ASSET_DIR=$(find "dist/Imaginify.app" -type d -path "*/customtkinter/assets/themes" -print 2>/dev/null | sed -n '1p')
if [ -n "$CTK_ASSET_DIR" ]; then
    echo "  OK - customtkinter theme assets found at: $CTK_ASSET_DIR"
else
    echo "  WARNING - customtkinter assets not detected inside the .app."
    echo "  The window may render blank. Try the manual test below."
fi
echo
echo "Test it now:  open dist/Imaginify.app"
echo
echo "If it still launches blank, run from Terminal to see error messages:"
echo "  dist/Imaginify.app/Contents/MacOS/Imaginify"
echo
echo "Drag Imaginify.app into /Applications to install."
echo "First launch: macOS may say 'unidentified developer' - right-click -> Open."
