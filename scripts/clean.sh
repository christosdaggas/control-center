#!/usr/bin/env bash
# Clean script for Control Center
set -euo pipefail

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$PROJECT_DIR"

echo "=== Cleaning Control Center ==="

# Clean cargo build artifacts
echo "Cleaning cargo build..."
cargo clean

# Clean dist directory
if [[ -d "dist" ]]; then
    echo "Cleaning dist directory..."
    rm -rf dist/*
fi

# Clean build directory (AppImage staging)
if [[ -d "build/appimage/AppDir" ]]; then
    echo "Cleaning AppImage build directory..."
    rm -rf build/appimage/AppDir
fi

echo "=== Clean complete ==="
