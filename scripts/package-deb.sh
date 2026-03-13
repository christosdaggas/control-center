#!/usr/bin/env bash
# DEB packaging script for Control Center
set -euo pipefail

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$PROJECT_DIR"

echo "=== Control Center DEB Package ==="

# Ensure release binary exists
if [[ ! -f "target/release/control-center" ]]; then
    echo "Release binary not found, building..."
    cargo build --release
fi

# Get version from Cargo.toml
VERSION=$(grep -m1 '^version' Cargo.toml | sed 's/.*"\(.*\)".*/\1/')
echo "Version: $VERSION"

# Create dist directory
mkdir -p dist/deb

# Install cargo-deb if needed
if ! cargo deb --version &>/dev/null; then
    echo "Installing cargo-deb..."
    cargo install cargo-deb
fi

# Generate DEB
echo "Generating DEB package..."
cargo deb --no-build

# Find and move the DEB
DEB_FILE=$(find target/debian -name "*.deb" -type f 2>/dev/null | head -1)
if [[ -n "$DEB_FILE" && -f "$DEB_FILE" ]]; then
    cp "$DEB_FILE" dist/deb/
    echo "DEB package created:"
    ls -lh dist/deb/*.deb
else
    echo "✗ DEB generation failed"
    exit 1
fi

echo "=== DEB packaging complete ==="
