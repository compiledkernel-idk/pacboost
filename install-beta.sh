#!/bin/bash
set -e

echo ":: Installing pacboost (BETA v1.2.0-beta)..."

# Create a temporary directory to avoid conflicts
TEMP_DIR=$(mktemp -d)
cd "$TEMP_DIR"

# Download the tarball
echo ":: Downloading..."
curl -sL -o pacboost.tar.gz https://github.com/compiledkernel-idk/pacboost/releases/download/v1.2.0-beta/pacboost-x86_64-linux.tar.gz

# Extract
echo ":: Extracting..."
tar -xzf pacboost.tar.gz

# Install
echo ":: Installing binary..."
if [ "$EUID" -ne 0 ]; then
  sudo mv pacboost /usr/local/bin/
else
  mv pacboost /usr/local/bin/
fi

# Cleanup
cd - > /dev/null
rm -rf "$TEMP_DIR"

echo ":: pacboost beta installed successfully!"
pacboost --version