#!/bin/bash
# ----------------------------------------------------------------
# Imaginify - macOS build script
# Produces dist/Imaginify.app  (a proper Mac app bundle)
# Run from Terminal:  bash build_macos.sh
# ----------------------------------------------------------------
set -e
cd "$(dirname "$0")"

echo "=== Imaginify macOS build ==="
echo

if ! command -v python3 >/dev/null 2>&1 ; then
    echo "Python 3 not found."
    echo "Install Python 3.10+ from https://www.python.org/downloads/macos/"
    echo "or run:  brew install python"
    exit 1
fi

echo "Installing build dependencies..."
python3 -m pip install --upgrade pip
python3 -m pip install Pillow customtkinter pyinstaller
echo

echo "Generating icon..."
python3 make_icon.py
echo

# Convert icon.png -> icon.icns for crisp Mac rendering
if command -v iconutil >/dev/null 2>&1 && command -v sips >/dev/null 2>&1 ; then
    echo "Converting icon to .icns ..."
    rm -rf icon.iconset
    mkdir icon.iconset
    sips -z 16 16     icon.png --out icon.iconset/icon_16x16.png       >/dev/null
    sips -z 32 32     icon.png --out icon.iconset/icon_16x16@2x.png    >/dev/null
    sips -z 32 32     icon.png --out icon.iconset/icon_32x32.png       >/dev/null
    sips -z 64 64     icon.png --out icon.iconset/icon_32x32@2x.png    >/dev/null
    sips -z 128 128   icon.png --out icon.iconset/icon_128x128.png     >/dev/null
    sips -z 256 256   icon.png --out icon.iconset/icon_128x128@2x.png  >/dev/null
    sips -z 256 256   icon.png --out icon.iconset/icon_256x256.png     >/dev/null
    sips -z 512 512   icon.png --out icon.iconset/icon_256x256@2x.png  >/dev/null
    sips -z 512 512   icon.png --out icon.iconset/icon_512x512.png     >/dev/null
    cp icon.png       icon.iconset/icon_512x512@2x.png
    iconutil -c icns icon.iconset
    ICON_FLAG="--icon icon.icns"
else
    echo "iconutil/sips not available - using PNG icon."
    ICON_FLAG="--icon icon.png"
fi
echo

echo "Cleaning previous build..."
rm -rf build dist Imaginify.spec
echo

echo "Building Imaginify.app with PyInstaller (this can take a minute)..."
python3 -m PyInstaller \
    --windowed \
    --name Imaginify \
    $ICON_FLAG \
    --collect-data customtkinter \
    --osx-bundle-identifier com.jezweb.imaginify \
    --clean \
    --noconfirm \
    imaginify.py

echo
echo "=== BUILD SUCCESSFUL ==="
echo
echo "Find your app at:  $(pwd)/dist/Imaginify.app"
echo
echo "First launch tip: macOS may say 'unidentified developer'."
echo "Right-click the app -> Open -> Open. After that it launches normally."
echo
echo "Optional: drag Imaginify.app into /Applications to install it system-wide."
