#!/bin/bash
set -e

# Slate-DE Installation Script
# This script installs dependencies, clones the repository, and builds the core components.

echo "Initializing Slate-DE installation..."

# 1. Dependency Check (Debian/Ubuntu focus, extensible to Arch/Fedora)
if [ -f /etc/debian_version ]; then
    echo "Detected Debian-based system. Installing dependencies..."
    sudo apt update
    sudo apt install -y build-essential pkg-config libwayland-dev libxkbcommon-dev \
        libdbus-1-dev libgbm-dev libinput-dev libudev-dev clang libclang-dev cmake
elif [ -f /etc/arch-release ]; then
    echo "Detected Arch Linux. Installing dependencies..."
    sudo pacman -S --needed base-devel wayland libxkbcommon dbus mesa libinput clang cmake
fi

# 2. Rustup Check
if ! command -v cargo &> /dev/null; then
    echo "Rust/Cargo not found. Installing via rustup..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
fi

# 3. Directory Setup
INSTALL_DIR="$HOME/.local/src/slate-de"
mkdir -p "$(dirname "$INSTALL_DIR")"

if [ -d "$INSTALL_DIR" ]; then
    echo "Updating existing Slate-DE repository..."
    cd "$INSTALL_DIR"
    git pull origin main
else
    echo "Cloning Slate-DE repository..."
    git clone --branch main https://github.com/flessan/slate-de.git "$INSTALL_DIR"
    cd "$INSTALL_DIR"
fi

# 4. Build
echo "Building Slate-DE CLI and Compositor..."
cargo build --release

# 5. Local Binary Symlinking
mkdir -p "$HOME/.local/bin"
ln -sf "$INSTALL_DIR/target/release/cli" "$HOME/.local/bin/slate-de"

echo "-------------------------------------------------------"
echo "Installation Complete."
echo "Binary linked to: $HOME/.local/bin/slate-de"
echo "Ensure $HOME/.local/bin is in your PATH."
echo "Run 'slate-de' to begin."
echo "-------------------------------------------------------"
