#!/bin/bash
/*
 * pacboost - High-performance Arch Linux package manager frontend.
 * Copyright (C) 2025  compiledkernel-idk and pacboost contributors
 */
set -e

REPO="compiledkernel-idk/pacboost"
LATEST_URL="https://api.github.com/repos/$REPO/releases/latest"

echo ":: Fetching latest release information..."
TAG=$(curl -s $LATEST_URL | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')

if [ -z "$TAG" ]; then
    echo "error: could not retrieve latest release tag"
    exit 1
fi

echo ":: Downloading pacboost $TAG..."
curl -L -# -o pacboost_bin "https://github.com/$REPO/releases/download/$TAG/pacboost"

echo ":: Downloading kdownload..."
curl -L -# -o kdownload_bin "https://github.com/$REPO/releases/download/$TAG/kdownload"

chmod +x pacboost_bin kdownload_bin

echo ":: Installing to /usr/local/bin (requires sudo)..."
sudo mv pacboost_bin /usr/local/bin/pacboost
sudo mv kdownload_bin /usr/local/bin/kdownload

echo ":: Installation successful."
echo "   You can now use 'pacboost' to manage your system."