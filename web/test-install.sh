#!/bin/bash
# Test script for Slate-DE installer
# This script simulates the installation in a temporary directory

set -e

TEST_DIR=$(mktemp -d)
echo "=== Testing Slate-DE Installer ==="
echo "Test directory: $TEST_DIR"
echo ""

# Create a mock home directory
export HOME="$TEST_DIR/mock_home"
mkdir -p "$HOME/.local/bin"
mkdir -p "$HOME/.config/slate"
mkdir -p "$HOME/.local/share"

echo "1. Testing dependency detection..."
# Mock OS detection
if [ -f /etc/os-release ]; then
    echo "   ✓ Detected OS: $(grep PRETTY_NAME /etc/os-release | cut -d'"' -f2)"
else
    echo "   ✗ Could not detect OS"
fi

echo ""
echo "2. Testing Rust installation check..."
if command -v cargo &> /dev/null; then
    echo "   ✓ Rust is installed: $(rustc --version)"
else
    echo "   ℹ Rust not installed (would install via rustup)"
fi

echo ""
echo "3. Testing directory creation..."
mkdir -p "$HOME/.local/share/slate-de"
mkdir -p "$HOME/.local/bin"
mkdir -p "$HOME/.config/slate"
echo "   ✓ Directories created successfully"

echo ""
echo "4. Testing symlink creation..."
touch "$HOME/.local/share/slate-de/test-binary"
ln -sf "$HOME/.local/share/slate-de/test-binary" "$HOME/.local/bin/slate-de"
if [ -x "$HOME/.local/bin/slate-de" ]; then
    echo "   ✓ Symlink created successfully"
else
    echo "   ✗ Symlink creation failed"
fi

echo ""
echo "5. Testing configuration creation..."
cat > "$HOME/.config/slate/slate.toml" << 'EOF'
[server]
socket_name = "slate-0"

[theme]
primary_color = "#88C0D0"
EOF
if [ -f "$HOME/.config/slate/slate.toml" ]; then
    echo "   ✓ Configuration file created"
else
    echo "   ✗ Configuration creation failed"
fi

echo ""
echo "=== Test Complete ==="
echo ""
echo "Cleaning up test directory..."
rm -rf "$TEST_DIR"
echo "Done!"
