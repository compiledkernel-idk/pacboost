#!/usr/bin/env bash
#
# pacboost - High-performance Arch Linux package manager frontend.
# Advanced Installation Script
#
# VERSION="2.3.3"
# Copyright (C) 2025 compiledkernel-idk and pacboost contributors
#
# This script installs pacboost safely and robustly.

set -euo pipefail

# Configuration
REPO="compiledkernel-idk/pacboost"
BINARY_NAME="pacboost"
INSTALL_DIR="/usr/bin"
TEMP_DIR="$(mktemp -d)"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
BOLD='\033[1m'
NC='\033[0m' # No Color

# Helper Functions
info() { echo -e "${BLUE}::${NC} ${BOLD}$1${NC}"; }
success() { echo -e "${GREEN}::${NC} ${BOLD}$1${NC}"; }
warn() { echo -e "${YELLOW}:: Warning:${NC} $1"; }
error() { echo -e "${RED}:: Error:${NC} $1"; exit 1; }

cleanup() {
    if [ -d "$TEMP_DIR" ]; then
        rm -rf "$TEMP_DIR"
    fi
}
trap cleanup EXIT

# 1. Pre-flight Checks
info "Checking system compatibility..."

# Architecture Check
ARCH=$(uname -m)
if [ "$ARCH" != "x86_64" ]; then
    error "Pacboost currently supports x86_64 only. Detected: $ARCH"
fi

# OS Check
OS=$(uname -s)
if [ "$OS" != "Linux" ]; then
    error "Pacboost requires Linux. Detected: $OS"
fi

# Distro Check (Soft check)
if [ ! -f "/etc/arch-release" ] && ! command -v pacman >/dev/null; then
    warn "This does not appear to be an Arch Linux-based system."
    warn "Pacboost is designed for Arch Linux (pacman/libalpm)."
    read -p "   Continue anyway? [y/N] " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        error "Installation aborted by user."
    fi
fi

# 2. Determine Version
info "Fetching release information..."

if [ -n "${PACBOOST_VERSION:-}" ]; then
    TAG="v${PACBOOST_VERSION#v}"
    info "Using specified version: $TAG"
    # Note: We assume the user knows what they are doing and the tag exists
else
    LATEST_URL="https://api.github.com/repos/$REPO/releases/latest"
    HTTP_RESPONSE=$(curl -sL -w "%{http_code}" -o "$TEMP_DIR/release.json" "$LATEST_URL")
    
    if [ "$HTTP_RESPONSE" != "200" ]; then
        # Fallback to local VERSION extraction if API fails (e.g. rate limit)
        # Note: This reads the VERSION variable from this script itself as a fallback assumption
        # that the script version matches the binary version, which is true for releases.
        FALLBACK_VER=$(grep '^# VERSION=' "$0" | cut -d'"' -f2)
        warn "Could not fetch latest release info from GitHub API (HTTP $HTTP_RESPONSE)."
        if [ -n "$FALLBACK_VER" ]; then
            TAG="v$FALLBACK_VER"
            warn "Falling back to script version: $TAG"
        else
            error "Could not determine version to download."
        fi
    else
        TAG=$(grep '"tag_name":' "$TEMP_DIR/release.json" | sed -E 's/.*"([^"]+)".*/\1/')
    fi
fi

if [ -z "$TAG" ]; then
    error "Failed to parse release tag."
fi

# 3. Download
TARBALL="pacboost-x86_64-linux.tar.gz"
DOWNLOAD_URL="https://github.com/$REPO/releases/download/$TAG/$TARBALL"

info "Downloading pacboost $TAG..."
info "Source: $DOWNLOAD_URL"

if command -v curl >/dev/null; then
    curl -L -# -o "$TEMP_DIR/$TARBALL" "$DOWNLOAD_URL"
elif command -v wget >/dev/null; then
    wget -q --show-progress -O "$TEMP_DIR/$TARBALL" "$DOWNLOAD_URL"
else
    error "Neither curl nor wget found. Please install one to continue."
fi

# Verify authenticity (Basic check that it's a valid tarball)
if ! tar -tzf "$TEMP_DIR/$TARBALL" >/dev/null 2>&1; then
    error "Downloaded file is not a valid tarball. Installation failed."
fi

# 4. Extract
info "Extracting..."
tar -xzf "$TEMP_DIR/$TARBALL" -C "$TEMP_DIR"

if [ ! -f "$TEMP_DIR/pacboost" ]; then
    error "Extraction failed: 'pacboost' binary not found in archive."
fi

# 5. Install
info "Installing to $INSTALL_DIR..."

# Check for write permissions or sudo
CAN_WRITE=0
if [ -w "$INSTALL_DIR" ]; then
    CAN_WRITE=1
fi

INSTALL_CMD="install -Dm755 $TEMP_DIR/pacboost $INSTALL_DIR/$BINARY_NAME"

if [ "$CAN_WRITE" -eq 1 ]; then
    $INSTALL_CMD
else
    if command -v sudo >/dev/null; then
        info "Requesting sudo permissions to install to $INSTALL_DIR..."
        sudo $INSTALL_CMD
    elif command -v doas >/dev/null; then
        info "Requesting doas permissions to install to $INSTALL_DIR..."
        doas $INSTALL_CMD
    elif [ "$(id -u)" -eq 0 ]; then
        $INSTALL_CMD
    else
        error "Insufficient permissions to install to $INSTALL_DIR and no sudo/doas found."
    fi
fi

# 6. Post-Install Verification
if command -v pacboost >/dev/null; then
    INSTALLED_VER=$(pacboost --version 2>/dev/null | head -n 1 | awk '{print $2}')
    success "Installation successful!"
    echo ""
    echo -e "   ${BOLD}pacboost $INSTALLED_VER${NC} is ready."
    echo ""
    echo -e "${BOLD}Usage Examples:${NC}"
    echo -e "   ${GREEN}pacboost${NC}               # Full system upgrade (sync + update)"
    echo -e "   ${GREEN}pacboost package${NC}       # Install 'package' (repo or AUR)"
    echo -e "   ${GREEN}pacboost --help${NC}        # View all commands"
else
    warn "Installation completed, but 'pacboost' is not in your PATH."
    warn "It was installed to: $INSTALL_DIR/pacboost"
    warn "You may need to add $INSTALL_DIR to your PATH or verify the installation."
fi

echo ""
success "Enjoy faster updates!"