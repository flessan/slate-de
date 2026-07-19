#!/bin/bash
# Slate-DE Standalone Installer
# This is a minimal bootstrap script that can be curl'd directly
# Usage: curl -sSf https://raw.githubusercontent.com/flessan/slate-de/main/install.sh | bash

set -e

echo "=== Slate-DE Quick Installer ==="
echo ""

# Download and run the full installer
INSTALL_SCRIPT=$(mktemp)
curl -sSf "https://raw.githubusercontent.com/flessan/slate-de/main/scripts/install.sh" -o "$INSTALL_SCRIPT"
chmod +x "$INSTALL_SCRIPT"
bash "$INSTALL_SCRIPT"

# Cleanup
rm -f "$INSTALL_SCRIPT"
