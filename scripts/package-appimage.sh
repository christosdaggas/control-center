#!/usr/bin/env bash
# AppImage packaging script for Control Center
set -euo pipefail

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$PROJECT_DIR"

echo "=== Control Center AppImage Package ==="

# Ensure release binary exists
if [[ ! -f "target/release/control-center" ]]; then
    echo "Release binary not found, building..."
    cargo build --release
fi

# Get version from Cargo.toml
VERSION=$(grep -m1 '^version' Cargo.toml | sed 's/.*"\(.*\)".*/\1/')
APP_NAME="Control_Center"
echo "Version: $VERSION"

# Create dist and build directories
mkdir -p dist/image
mkdir -p build/appimage

# Create AppDir structure
APPDIR="build/appimage/AppDir"
rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin"
mkdir -p "$APPDIR/usr/share/applications"
mkdir -p "$APPDIR/usr/share/metainfo"
mkdir -p "$APPDIR/usr/share/icons/hicolor/scalable/apps"
mkdir -p "$APPDIR/usr/share/control-center"

# Copy binary
cp target/release/control-center "$APPDIR/usr/bin/"

# Copy desktop file
cp data/com.chrisdaggas.control-center.desktop "$APPDIR/usr/share/applications/"
cp data/com.chrisdaggas.control-center.desktop "$APPDIR/"

# Copy metainfo
cp data/com.chrisdaggas.control-center.metainfo.xml "$APPDIR/usr/share/metainfo/"

# Copy icons
cp data/icons/hicolor/scalable/apps/com.chrisdaggas.control-center.svg "$APPDIR/usr/share/icons/hicolor/scalable/apps/"
cp data/icons/hicolor/scalable/apps/com.chrisdaggas.control-center.svg "$APPDIR/com.chrisdaggas.control-center.svg"

# Create AppRun
cat > "$APPDIR/AppRun" << 'APPRUN'
#!/bin/bash
SELF=$(readlink -f "$0")
HERE=${SELF%/*}
export PATH="${HERE}/usr/bin:${PATH}"
export LD_LIBRARY_PATH="${HERE}/usr/lib:${LD_LIBRARY_PATH:-}"
export XDG_DATA_DIRS="${HERE}/usr/share:${XDG_DATA_DIRS:-/usr/share}"
exec "${HERE}/usr/bin/control-center" "$@"
APPRUN
chmod +x "$APPDIR/AppRun"

# Download appimagetool if needed
APPIMAGETOOL="build/appimage/appimagetool"
if [[ ! -x "$APPIMAGETOOL" ]]; then
    echo "Downloading appimagetool..."
    wget -q -O "$APPIMAGETOOL" "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage"
    chmod +x "$APPIMAGETOOL"
fi

# Create AppImage
echo "Creating AppImage..."
APPIMAGE_NAME="${APP_NAME}-${VERSION}-x86_64.AppImage"
ARCH=x86_64 "$APPIMAGETOOL" --appimage-extract-and-run "$APPDIR" "dist/image/${APPIMAGE_NAME}"

if [[ -f "dist/image/${APPIMAGE_NAME}" ]]; then
    chmod +x "dist/image/${APPIMAGE_NAME}"
    echo "✓ AppImage created:"
    ls -lh "dist/image/${APPIMAGE_NAME}"
else
    echo "✗ AppImage creation failed"
    exit 1
fi

echo "=== AppImage packaging complete ==="
