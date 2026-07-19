#!/bin/bash
# Slate-DE Uninstaller
# This script removes Slate-DE from your system

set -e

SLATE_INSTALL_DIR="$HOME/.local/share/slate-de"
SLATE_BIN_DIR="$HOME/.local/bin"
SLATE_CONFIG_DIR="$HOME/.config/slate"

echo "=== Slate-DE Uninstaller ==="
echo ""
echo "This will remove:"
echo "  - $SLATE_INSTALL_DIR"
echo "  - $SLATE_BIN_DIR/slate-de"
echo "  - $SLATE_BIN_DIR/slate"
echo "  - $SLATE_BIN_DIR/slate-session"
echo ""
read -p "Are you sure you want to continue? (y/N) " -n 1 -r
echo ""

if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo "Uninstallation cancelled."
    exit 0
fi

echo ""
echo "Removing Slate-DE..."

# Remove binaries
rm -f "$SLATE_BIN_DIR/slate-de"
rm -f "$SLATE_BIN_DIR/slate"
rm -f "$SLATE_BIN_DIR/slate-session"
echo "  ✓ Removed binaries"

# Remove installation directory
if [ -d "$SLATE_INSTALL_DIR" ]; then
    rm -rf "$SLATE_INSTALL_DIR"
    echo "  ✓ Removed installation directory"
fi

# Ask about config
echo ""
read -p "Remove configuration directory ($SLATE_CONFIG_DIR)? (y/N) " -n 1 -r
echo ""
if [[ $REPLY =~ ^[Yy]$ ]]; then
    rm -rf "$SLATE_CONFIG_DIR"
    echo "  ✓ Removed configuration"
fi

# Ask about data
echo ""
read -p "Remove plugin and log data? (y/N) " -n 1 -r
echo ""
if [[ $REPLY =~ ^[Yy]$ ]]; then
    rm -rf "$HOME/.local/share/slate-de"
    echo "  ✓ Removed data directory"
fi

echo ""
echo "=== Uninstallation Complete ==="
echo ""
echo "Slate-DE has been removed from your system."
echo ""
echo "Note: Rust and system dependencies were not removed."
echo "      You can uninstall them manually if no longer needed."
