#!/bin/bash
#
# pacboost - High-performance Arch Linux package manager frontend.
# Version: 1.5.0
# Updated: 2025-12-24
# Copyright (C) 2025  compiledkernel-idk and pacboost contributors
#
set -e

REPO="compiledkernel-idk/pacboost"
LATEST_URL="https://api.github.com/repos/$REPO/releases/latest"

echo ":: Fetching latest release information..."
TAG=$(curl -s $LATEST_URL | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')

if [ -z "$TAG" ]; then
    echo "error: could not retrieve latest release tag"
    exit 1
fi

echo ":: Found version: $TAG"

TARBALL="pacboost-x86_64-linux.tar.gz"
URL="https://github.com/$REPO/releases/download/$TAG/$TARBALL"

echo ":: Downloading $TARBALL..."
curl -L -# -o "$TARBALL" "$URL"

echo ":: Extracting binary..."
TMP_DIR=$(mktemp -d)
tar -xzf "$TARBALL" -C "$TMP_DIR"

echo ":: Installing to /usr/local/bin (requires sudo)..."
sudo install -Dm755 "$TMP_DIR/pacboost" /usr/local/bin/pacboost

echo ":: Cleaning up..."
rm "$TARBALL"
rm -rf "$TMP_DIR"

echo ""
echo ":: Installation successful!"
echo "   pacboost $TAG installed to /usr/local/bin/pacboost"
echo ""
echo "   Features in v1.5.0:"
echo "     - Added easter egg for self-installation"
echo "     - Published to AUR (pacboost and pacboost-bin)"
echo "     - Automatic updates from latest master/release"
echo "     - Complete AUR subsystem with dependency resolution"
echo "     - Security scanning for PKGBUILDs"
echo "     - Automatic PGP key importing"
echo "     - Enhanced UI with progress bars and detailed output"
echo ""
echo "   Usage: sudo pacboost -Syu        # Full system upgrade"
echo "          sudo pacboost -S <pkg>    # Install package (official or AUR)"
echo "          pacboost -A <query>       # Search AUR"
echo "          Thank you for using pacboost!" 