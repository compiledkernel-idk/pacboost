#!/bin/bash
set -e

echo ":: Installing pacboost (BETA v1.2.0-beta)..."

# Download the tarball
curl -L -o pacboost.tar.gz https://github.com/compiledkernel-idk/pacboost/releases/download/v1.2.0-beta/pacboost-x86_64-linux.tar.gz

# Extract
tar -xzf pacboost.tar.gz

# Install
echo ":: Installing binary..."
sudo mv pacboost /usr/local/bin/

# Cleanup
rm pacboost.tar.gz

echo ":: pacboost beta installed successfully!"
pacboost --version
