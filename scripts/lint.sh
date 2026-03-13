#!/usr/bin/env bash
# Lint script for Control Center
set -euo pipefail

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$PROJECT_DIR"

echo "=== Control Center Lint Check ==="

echo ""
echo "Running cargo clippy..."
cargo clippy --release -- -D warnings

echo ""
echo "Running cargo fmt check..."
cargo fmt -- --check

echo ""
echo "=== Lint check complete ==="
