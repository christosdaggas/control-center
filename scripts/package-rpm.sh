#!/usr/bin/env bash
# RPM packaging script for Control Center
set -euo pipefail

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$PROJECT_DIR"

echo "=== Control Center RPM Package ==="

# Ensure release binary exists
if [[ ! -f "target/release/control-center" ]]; then
    echo "Release binary not found, building..."
    cargo build --release
fi

# Get version from Cargo.toml
VERSION=$(grep -m1 '^version' Cargo.toml | sed 's/.*"\(.*\)".*/\1/')
echo "Version: $VERSION"

# Create dist directory
mkdir -p dist/rpm

# Install cargo-generate-rpm if needed
if ! cargo generate-rpm --version &>/dev/null; then
    echo "Installing cargo-generate-rpm..."
    cargo install cargo-generate-rpm
fi

# Generate RPM
echo "Generating RPM package..."
cargo generate-rpm

# Find and move the RPM
RPM_FILE=$(find target/generate-rpm -name "*.rpm" -type f 2>/dev/null | sort -V | tail -1)
if [[ -n "$RPM_FILE" && -f "$RPM_FILE" ]]; then
    cp "$RPM_FILE" dist/rpm/
    echo "RPM package created:"
    ls -lh dist/rpm/*.rpm
else
    echo "✗ RPM generation failed"
    exit 1
fi

echo "=== RPM packaging complete ==="
