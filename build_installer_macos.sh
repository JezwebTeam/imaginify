#!/bin/bash
# ----------------------------------------------------------------
#  Imaginify - macOS installer build (.pkg wizard)
#  Produces: dist/Imaginify-<version>.pkg
#  Wraps:    dist/Imaginify.app (built by build_macos.sh)
#
#  Wizard pages: Welcome -> License -> Destination -> Install -> Summary
#  All built using Apple's pkgbuild + productbuild (no extra tools).
#
#  Run from Terminal:  bash build_installer_macos.sh
# ----------------------------------------------------------------
set -e
cd "$(dirname "$0")"

APP_NAME="Imaginify"
APP_VERSION="1.0.0"
BUNDLE_ID="com.jezweb.imaginify"
APP_BUNDLE="dist/${APP_NAME}.app"
COMPONENT_PKG="dist/${APP_NAME}-component.pkg"
FINAL_PKG="dist/${APP_NAME}-${APP_VERSION}.pkg"
STAGING_DIR="dist/_pkg_staging"

echo "=== Imaginify macOS installer build ==="
echo

if [[ "$(uname)" != "Darwin" ]]; then
    echo "This script must be run on macOS (it uses pkgbuild and productbuild)."
    exit 1
fi

if [[ ! -d "$APP_BUNDLE" ]]; then
    echo "$APP_BUNDLE not found."
    echo "Run  bash build_macos.sh  first to build the .app, then re-run this script."
    exit 1
fi

if [[ ! -f LICENSE.txt ]]; then
    echo "LICENSE.txt is missing from the project root."
    exit 1
fi

if [[ ! -f installer_macos/distribution.xml ]]; then
    echo "installer_macos/distribution.xml is missing."
    exit 1
fi

echo "Preparing staging directory at $STAGING_DIR ..."
rm -rf "$STAGING_DIR"
mkdir -p "$STAGING_DIR"
cp -R "$APP_BUNDLE" "$STAGING_DIR/"
echo

echo "Copying LICENSE.txt into installer resources ..."
cp LICENSE.txt installer_macos/resources/LICENSE.txt
echo

echo "Building component package (the actual files) ..."
pkgbuild \
    --root "$STAGING_DIR" \
    --identifier "$BUNDLE_ID" \
    --version "$APP_VERSION" \
    --install-location /Applications \
    "$COMPONENT_PKG"
echo

echo "Building distribution package (the wizard) ..."
productbuild \
    --distribution installer_macos/distribution.xml \
    --resources    installer_macos/resources \
    --package-path dist \
    "$FINAL_PKG"
echo

echo "Cleaning intermediate files ..."
rm -f "$COMPONENT_PKG"
rm -rf "$STAGING_DIR"
rm -f  installer_macos/resources/LICENSE.txt
echo

echo "=== INSTALLER BUILD SUCCESSFUL ==="
echo
echo "Output: $(pwd)/$FINAL_PKG"
echo
echo "Test it by double-clicking the .pkg in Finder, or running:"
echo "  open \"$FINAL_PKG\""
echo
echo "Note: an unsigned .pkg is blocked by Gatekeeper on first launch."
echo "Users need to right-click the .pkg -> Open -> Open. After that it installs normally."
echo "To avoid this prompt, sign and notarize the .pkg with an Apple Developer ID."
