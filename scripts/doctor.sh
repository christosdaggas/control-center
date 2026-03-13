#!/usr/bin/env bash
# Doctor script for Control Center - checks build dependencies
set -euo pipefail

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$PROJECT_DIR"

echo "=== Control Center Dependency Check ==="

check_command() {
    local cmd="$1"
    local pkg="${2:-$1}"
    if command -v "$cmd" &>/dev/null; then
        echo "✓ $cmd is installed"
        return 0
    else
        echo "✗ $cmd is NOT installed (install: $pkg)"
        return 1
    fi
}

check_lib() {
    local lib="$1"
    local pkg="${2:-$1}"
    if pkg-config --exists "$lib" 2>/dev/null; then
        local version=$(pkg-config --modversion "$lib" 2>/dev/null || echo "unknown")
        echo "✓ $lib ($version)"
        return 0
    else
        echo "✗ $lib is NOT installed (install: $pkg)"
        return 1
    fi
}

ERRORS=0

echo ""
echo "Required commands:"
check_command cargo rust || ((ERRORS++))
check_command rustc rust || ((ERRORS++))
check_command pkg-config pkg-config || ((ERRORS++))

echo ""
echo "Required libraries:"
check_lib gtk4 gtk4-devel || ((ERRORS++))
check_lib libadwaita-1 libadwaita-devel || ((ERRORS++))

echo ""
echo "Optional (for packaging):"
check_command wget wget || true
check_command rpmbuild rpm-build || true
check_command dpkg-deb dpkg || true

echo ""
if [[ $ERRORS -eq 0 ]]; then
    echo "=== All required dependencies present ==="
    exit 0
else
    echo "=== $ERRORS missing dependencies ==="
    exit 1
fi
