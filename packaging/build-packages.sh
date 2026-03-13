#!/bin/bash
set -e

# Control Center - Package Build Script
# Creates .rpm, .deb, and .appimage packages

APP_NAME="control-center"
PKG_NAME="lnx-control-center"
APP_ID="com.chrisdaggas.control-center"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
BUILD_DIR="$PROJECT_DIR/build"
DIST_DIR="$PROJECT_DIR/dist"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Extract version robustly
get_version() {
    if command -v cargo >/dev/null 2>&1 && command -v jq >/dev/null 2>&1; then
        cargo metadata --format-version 1 --no-deps | jq -r '.packages[0].version'
    else
        # Fallback to sed
        sed -n 's/^version = "\(.*\)"/\1/p' "$PROJECT_DIR/Cargo.toml" | head -n1
    fi
}

VERSION=$(get_version)

if [ -z "$VERSION" ]; then
    log_error "Failed to extract version from Cargo.toml"
    exit 1
fi

log_info "Building version: $VERSION"

# Check for required tools
check_deps() {
    log_info "Checking dependencies..."
    
    local missing=()
    
    command -v cargo >/dev/null 2>&1 || missing+=("cargo")
    command -v strip >/dev/null 2>&1 || missing+=("strip (binutils)")
    
    if [ ${#missing[@]} -ne 0 ]; then
        log_error "Missing required tools: ${missing[*]}"
        exit 1
    fi
    
    log_info "All required tools found."
}

# Build the release binary
build_release() {
    log_info "Building release binary..."
    cd "$PROJECT_DIR"
    cargo build --release
    strip target/release/$APP_NAME
    log_info "Binary built and stripped: $(ls -lh target/release/$APP_NAME | awk '{print $5}')"
}

# Create directory structure for packaging
prepare_dirs() {
    log_info "Preparing build directories..."
    rm -rf "$BUILD_DIR" "$DIST_DIR"
    mkdir -p "$BUILD_DIR"
    mkdir -p "$DIST_DIR"/{app,image,deb,rpm}
}

# Build DEB package
build_deb() {
    log_info "Building DEB package..."
    
    if command -v cargo-deb >/dev/null 2>&1; then
        log_info "Using cargo-deb..."
        cargo deb --output "$DIST_DIR/deb/${PKG_NAME}_${VERSION}_amd64.deb"
        return $?
    fi

    if ! command -v dpkg-deb >/dev/null 2>&1; then
        log_warn "dpkg-deb not found, skipping DEB package"
        return 1
    fi
    
    local DEB_DIR="$BUILD_DIR/deb"
    local DEB_ROOT="$DEB_DIR/${PKG_NAME}_${VERSION}_amd64"
    
    mkdir -p "$DEB_ROOT/DEBIAN"
    mkdir -p "$DEB_ROOT/usr/bin"
    mkdir -p "$DEB_ROOT/usr/share/applications"
    mkdir -p "$DEB_ROOT/usr/share/metainfo"
    mkdir -p "$DEB_ROOT/usr/share/icons/hicolor/scalable/apps"
    mkdir -p "$DEB_ROOT/usr/share/doc/$PKG_NAME"
    
    # Copy binary
    cp "$PROJECT_DIR/target/release/$APP_NAME" "$DEB_ROOT/usr/bin/"
    chmod 755 "$DEB_ROOT/usr/bin/$APP_NAME"
    
    # Copy data files
    cp "$PROJECT_DIR/data/$APP_ID.desktop" "$DEB_ROOT/usr/share/applications/"
    cp "$PROJECT_DIR/data/$APP_ID.metainfo.xml" "$DEB_ROOT/usr/share/metainfo/"
    cp "$PROJECT_DIR/data/icons/hicolor/scalable/apps/$APP_ID.svg" "$DEB_ROOT/usr/share/icons/hicolor/scalable/apps/"
    cp "$PROJECT_DIR/README.md" "$DEB_ROOT/usr/share/doc/$PKG_NAME/"
    
    # Calculate installed size
    INSTALLED_SIZE=$(du -sk "$DEB_ROOT" | cut -f1)
    
    # Create control file
    cat > "$DEB_ROOT/DEBIAN/control" << EOF
Package: $PKG_NAME
Version: $VERSION
Section: utils
Priority: optional
Architecture: amd64
Installed-Size: $INSTALLED_SIZE
Depends: libgtk-4-1, libadwaita-1-0
Maintainer: Christos A. Daggas <info@chrisdaggas.com>
Description: Linux System Change & Activity Timeline Viewer
 Control Center is a modern GTK4/Libadwaita desktop application for monitoring
 system health, managing services, viewing activity logs, and comparing
 system state snapshots.
Homepage: https://chrisdaggas.com
EOF
    
    # Create post-install script
    cat > "$DEB_ROOT/DEBIAN/postinst" << 'EOF'
#!/bin/bash
set -e
gtk-update-icon-cache -f /usr/share/icons/hicolor 2>/dev/null || true
update-desktop-database /usr/share/applications 2>/dev/null || true
EOF
    chmod 755 "$DEB_ROOT/DEBIAN/postinst"
    
    # Build package
    dpkg-deb --build --root-owner-group "$DEB_ROOT"
    mv "$DEB_ROOT.deb" "$DIST_DIR/deb/${PKG_NAME}_${VERSION}_amd64.deb"
    
    log_info "DEB package created: $DIST_DIR/deb/${PKG_NAME}_${VERSION}_amd64.deb"
}

# Build RPM package
build_rpm() {
    log_info "Building RPM package..."
    
    if command -v cargo-generate-rpm >/dev/null 2>&1; then
        log_info "Using cargo-generate-rpm..."
        cargo generate-rpm --target-dir "target"
        # Find and move the generated RPM
        find "target/generate-rpm" -name "*.rpm" -exec cp {} "$DIST_DIR/rpm/" \;
        return 0
    fi

    if ! command -v rpmbuild >/dev/null 2>&1; then
        log_warn "rpmbuild not found, skipping RPM package"
        return 1
    fi
    
    local RPM_DIR="/tmp/rpmbuild-${APP_NAME}"
    rm -rf "$RPM_DIR"
    mkdir -p "$RPM_DIR"/{BUILD,RPMS,SOURCES,SPECS,SRPMS,BUILDROOT}
    
    local RPM_ROOT="$RPM_DIR/root"
    mkdir -p "$RPM_ROOT/usr/bin"
    mkdir -p "$RPM_ROOT/usr/share/applications"
    mkdir -p "$RPM_ROOT/usr/share/metainfo"
    mkdir -p "$RPM_ROOT/usr/share/icons/hicolor/scalable/apps"
    mkdir -p "$RPM_ROOT/usr/share/doc/$PKG_NAME"
    
    cp "$PROJECT_DIR/target/release/$APP_NAME" "$RPM_ROOT/usr/bin/"
    cp "$PROJECT_DIR/data/$APP_ID.desktop" "$RPM_ROOT/usr/share/applications/"
    cp "$PROJECT_DIR/data/$APP_ID.metainfo.xml" "$RPM_ROOT/usr/share/metainfo/"
    cp "$PROJECT_DIR/data/icons/hicolor/scalable/apps/$APP_ID.svg" "$RPM_ROOT/usr/share/icons/hicolor/scalable/apps/"
    cp "$PROJECT_DIR/README.md" "$RPM_ROOT/usr/share/doc/$PKG_NAME/"
    
    # Create spec file
    cat > "$RPM_DIR/SPECS/${PKG_NAME}.spec" << EOF
%define _build_id_links none
%define debug_package %{nil}

Name:           $PKG_NAME
Version:        $VERSION
Release:        1%{?dist}
Summary:        Linux System Change & Activity Timeline Viewer

License:        MIT
URL:            https://chrisdaggas.com

Requires:       gtk4
Requires:       libadwaita

%description
Control Center is a modern GTK4/Libadwaita desktop application for monitoring
system health, managing services, viewing activity logs, and comparing
system state snapshots.

%prep
# Nothing to prepare

%build
# Nothing to build

%install
mkdir -p %{buildroot}
cp -a "${RPM_ROOT}"/* %{buildroot}/

%files
%{_bindir}/$APP_NAME
%{_datadir}/applications/$APP_ID.desktop
%{_datadir}/metainfo/$APP_ID.metainfo.xml
%{_datadir}/icons/hicolor/scalable/apps/$APP_ID.svg
%{_datadir}/doc/$PKG_NAME/README.md

%post
gtk-update-icon-cache -f /usr/share/icons/hicolor 2>/dev/null || true
update-desktop-database /usr/share/applications 2>/dev/null || true

%changelog
* $(date '+%a %b %d %Y') Christos A. Daggas <info@chrisdaggas.com> - ${VERSION}-1
- Release ${VERSION}
EOF
    
    rpmbuild --define "_topdir $RPM_DIR" \
             -bb "$RPM_DIR/SPECS/${PKG_NAME}.spec"
    
    find "$RPM_DIR/RPMS" -name "*.rpm" -exec mv {} "$DIST_DIR/rpm/" \;
    
    log_info "RPM package created in $DIST_DIR/rpm/"
}

# Build AppImage
build_appimage() {
    log_info "Building AppImage..."
    
    local APPIMAGE_DIR="$BUILD_DIR/appimage"
    local APPDIR="$APPIMAGE_DIR/AppDir"
    
    mkdir -p "$APPDIR/usr/bin"
    mkdir -p "$APPDIR/usr/share/applications"
    mkdir -p "$APPDIR/usr/share/metainfo"
    mkdir -p "$APPDIR/usr/share/icons/hicolor/scalable/apps"
    mkdir -p "$APPDIR/usr/lib"
    
    # Copy binary
    cp "$PROJECT_DIR/target/release/$APP_NAME" "$APPDIR/usr/bin/"
    chmod 755 "$APPDIR/usr/bin/$APP_NAME"
    
    # Copy data files
    cp "$PROJECT_DIR/data/$APP_ID.desktop" "$APPDIR/usr/share/applications/"
    cp "$PROJECT_DIR/data/$APP_ID.desktop" "$APPDIR/$APP_ID.desktop"
    cp "$PROJECT_DIR/data/$APP_ID.metainfo.xml" "$APPDIR/usr/share/metainfo/"
    cp "$PROJECT_DIR/data/icons/hicolor/scalable/apps/$APP_ID.svg" "$APPDIR/usr/share/icons/hicolor/scalable/apps/"
    cp "$PROJECT_DIR/data/icons/hicolor/scalable/apps/$APP_ID.svg" "$APPDIR/$APP_ID.svg"
    
    # Create AppRun script
    cat > "$APPDIR/AppRun" << 'EOF'
#!/bin/bash
SELF=$(readlink -f "$0")
HERE=${SELF%/*}

# Set up environment
export PATH="${HERE}/usr/bin:${PATH}"
export LD_LIBRARY_PATH="${HERE}/usr/lib:${LD_LIBRARY_PATH}"
export XDG_DATA_DIRS="${HERE}/usr/share:${XDG_DATA_DIRS}"
export GSETTINGS_SCHEMA_DIR="${HERE}/usr/share/glib-2.0/schemas:${GSETTINGS_SCHEMA_DIR}"

# Run the application
exec "${HERE}/usr/bin/control-center" "$@"
EOF
    chmod 755 "$APPDIR/AppRun"
    
    # Create .DirIcon symlink
    ln -sf "$APP_ID.svg" "$APPDIR/.DirIcon"
    
    # Check for appimagetool
    if ! command -v appimagetool >/dev/null 2>&1; then
        log_warn "appimagetool not found. Downloading..."
        
        APPIMAGETOOL="$BUILD_DIR/appimagetool"
        # Use a pinned version or check logic
        wget -q "https://github.com/AppImage/AppImageKit/releases/download/13/appimagetool-x86_64.AppImage" -O "$APPIMAGETOOL"
        chmod +x "$APPIMAGETOOL"
        
        # Verify it works (sometimes requires fuse)
        # If fuse is missing, extract it
        if ! "$APPIMAGETOOL" --version >/dev/null 2>&1; then
             log_warn "AppImage requires FUSE, forcing extraction"
             cd "$BUILD_DIR"
             "$APPIMAGETOOL" --appimage-extract >/dev/null 2>&1
             APPIMAGETOOL="$BUILD_DIR/squashfs-root/AppRun"
        fi
    else
        APPIMAGETOOL="appimagetool"
    fi
    
    # Build AppImage
    cd "$APPIMAGE_DIR"
    ARCH=x86_64 "$APPIMAGETOOL" --no-appstream "$APPDIR" "$DIST_DIR/image/${APP_NAME}-${VERSION}-x86_64.AppImage"
    
    log_info "AppImage created: $DIST_DIR/image/${APP_NAME}-${VERSION}-x86_64.AppImage"
}

# Main
main() {
    echo ""
    echo "╔══════════════════════════════════════════╗"
    echo "║     Control Center Package Builder       ║"
    echo "║           Version $VERSION                  ║"
    echo "╚══════════════════════════════════════════╝"
    echo ""
    
    check_deps
    build_release
    prepare_dirs
    
    # Build all packages
    build_deb || log_warn "Failed to build DEB"
    build_rpm || log_warn "Failed to build RPM"
    build_appimage || log_warn "Failed to build AppImage"
    
    echo ""
    log_info "Build complete! Packages available in: $DIST_DIR/"
    echo "=== DEB ==="
    ls -lh "$DIST_DIR/deb/" 2>/dev/null || true
    echo "=== RPM ==="
    ls -lh "$DIST_DIR/rpm/" 2>/dev/null || true
    echo "=== AppImage ==="
    ls -lh "$DIST_DIR/image/" 2>/dev/null || true
    echo ""
}

# Run
main "$@"
