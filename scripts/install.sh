#!/bin/bash
set -e

# Slate-DE One-Click Installation Script
# This script provides a complete, automated installation of Slate-DE
# Usage: curl -sSf https://raw.githubusercontent.com/flessan/slate-de/main/scripts/install.sh | bash

readonly SLATE_VERSION="0.1.0"
readonly SLATE_REPO="https://github.com/flessan/slate-de.git"
readonly SLATE_INSTALL_DIR="$HOME/.local/share/slate-de"
readonly SLATE_BIN_DIR="$HOME/.local/bin"
readonly SLATE_CONFIG_DIR="$HOME/.config/slate"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if running on supported OS
check_os() {
    if [[ "$OSTYPE" != "linux-gnu"* ]]; then
        log_error "Slate-DE is only supported on Linux. Detected: $OSTYPE"
        exit 1
    fi
    log_info "Operating System: Linux ✓"
}

# Detect distribution
detect_distro() {
    if [ -f /etc/os-release ]; then
        . /etc/os-release
        DISTRO=$ID
        log_info "Detected distribution: $PRETTY_NAME"
    elif [ -f /etc/debian_version ]; then
        DISTRO="debian"
        log_info "Detected distribution: Debian"
    elif [ -f /etc/arch-release ]; then
        DISTRO="arch"
        log_info "Detected distribution: Arch Linux"
    elif [ -f /etc/fedora-release ]; then
        DISTRO="fedora"
        log_info "Detected distribution: Fedora"
    else
        DISTRO="unknown"
        log_warn "Could not detect distribution. Attempting generic installation."
    fi
}

# Install system dependencies based on distribution
install_dependencies() {
    log_info "Installing system dependencies..."

    case $DISTRO in
        "ubuntu"|"debian"|"pop"|"elementary"|"linuxmint"|"zorin")
            sudo apt update
            sudo apt install -y \
                build-essential \
                pkg-config \
                libwayland-dev \
                libxkbcommon-dev \
                libdbus-1-dev \
                libgbm-dev \
                libinput-dev \
                libudev-dev \
                clang \
                libclang-dev \
                cmake \
                git \
                curl \
                rustup
            ;;
        "arch"|"manjaro"|"endeavouros"|"cachyos")
            sudo pacman -S --needed --noconfirm \
                base-devel \
                wayland \
                libxkbcommon \
                dbus \
                mesa \
                libinput \
                clang \
                cmake \
                git \
                curl \
                rustup
            ;;
        "fedora"|"rhel"|"centos")
            sudo dnf install -y \
                gcc \
                gcc-c++ \
                make \
                pkg-config \
                wayland-devel \
                libxkbcommon-devel \
                dbus-devel \
                mesa-libgbm-devel \
                libinput-devel \
                systemd-devel \
                clang \
                cmake \
                git \
                curl \
                rustup
            ;;
        *)
            log_warn "Unknown distribution. Please install these packages manually:"
            log_warn "  - build tools (gcc, make, pkg-config)"
            log_warn "  - Wayland libraries (wayland-dev, libxkbcommon-dev)"
            log_warn "  - Graphics libraries (libgbm-dev, mesa-dev)"
            log_warn "  - Input libraries (libinput-dev, libudev-dev)"
            log_warn "  - clang/LLVM for bindings"
            log_warn "  - git, curl"
            read -p "Press enter to continue anyway, or Ctrl+C to abort..."
            ;;
    esac

    log_success "System dependencies installed"
}

# Install Rust if not present
install_rust() {
    if command -v cargo &> /dev/null; then
        log_info "Rust/Cargo already installed ✓"
        rustc --version
        return
    fi

    log_info "Installing Rust toolchain..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable

    # Source cargo environment
    . "$HOME/.cargo/env"

    log_success "Rust installed successfully"
    rustc --version
}

# Setup directories
setup_directories() {
    log_info "Setting up directories..."

    mkdir -p "$SLATE_INSTALL_DIR"
    mkdir -p "$SLATE_BIN_DIR"
    mkdir -p "$SLATE_CONFIG_DIR"
    mkdir -p "$HOME/.local/share/slate-de/plugins"
    mkdir -p "$HOME/.local/share/slate-de/logs"

    # Add ~/.local/bin to PATH if not already present
    if [[ ":$PATH:" != *":$HOME/.local/bin:"* ]]; then
        echo 'export PATH="$HOME/.local/bin:$PATH"' >> "$HOME/.bashrc"
        echo 'export PATH="$HOME/.local/bin:$PATH"' >> "$HOME/.profile"
        export PATH="$HOME/.local/bin:$PATH"
    fi

    log_success "Directories created"
}

# Clone or update repository
setup_repository() {
    if [ -d "$SLATE_INSTALL_DIR/.git" ]; then
        log_info "Updating existing Slate-DE installation..."
        cd "$SLATE_INSTALL_DIR"
        git fetch origin
        git checkout main
        git pull origin main
    else
        log_info "Cloning Slate-DE repository..."
        rm -rf "$SLATE_INSTALL_DIR" 2>/dev/null || true
        git clone "$SLATE_REPO" "$SLATE_INSTALL_DIR"
        cd "$SLATE_INSTALL_DIR"
    fi

    log_success "Repository ready"
}

# Build Slate-DE
build_slate() {
    log_info "Building Slate-DE (this may take a few minutes)..."

    cd "$SLATE_INSTALL_DIR"

    # Ensure we have the correct Rust toolchain
    if [ -f "$HOME/.cargo/env" ]; then
        . "$HOME/.cargo/env"
    fi

    # Build in release mode
    cargo build --release

    log_success "Build complete"
}

# Create symlinks and wrapper scripts
setup_executables() {
    log_info "Setting up executables..."

    # Remove old symlinks if they exist
    rm -f "$SLATE_BIN_DIR/slate-de"
    rm -f "$SLATE_BIN_DIR/slate"

    # Create symlink to the main binary
    ln -sf "$SLATE_INSTALL_DIR/target/release/cli" "$SLATE_BIN_DIR/slate-de"

    # Create a simpler 'slate' alias
    ln -sf "$SLATE_INSTALL_DIR/target/release/cli" "$SLATE_BIN_DIR/slate"

    # Create a launcher script for Wayland session
    cat > "$SLATE_BIN_DIR/slate-session" << 'EOF'
#!/bin/bash
# Slate-DE Session Launcher

export XDG_SESSION_TYPE=wayland
export XDG_CURRENT_DESKTOP=slate-de
export QT_QPA_PLATFORM=wayland
export QT_WAYLAND_DISABLE_WINDOWDECORATION=1
export MOZ_ENABLE_WAYLAND=1
export ELECTRON_OZONE_PLATFORM_HINT=wayland

# Start Slate-DE
exec slate-de "$@"
EOF
    chmod +x "$SLATE_BIN_DIR/slate-session"

    log_success "Executables installed to $SLATE_BIN_DIR"
}

# Create default configuration
create_default_config() {
    if [ -f "$SLATE_CONFIG_DIR/slate.toml" ]; then
        log_warn "Configuration file already exists at $SLATE_CONFIG_DIR/slate.toml"
        return
    fi

    log_info "Creating default configuration..."

    cat > "$SLATE_CONFIG_DIR/slate.toml" << 'EOF'
# Slate-DE Configuration

[server]
socket_name = "slate-0"

[theme]
primary_color = "#88C0D0"
background_color = "#2E3440"
text_color = "#D8DEE9"

[compositor]
# Enable/disable vsync
vsync = true

# Default output configuration
[compositor.outputs]
# Outputs are auto-detected, but can be configured here

[workspaces]
# Default workspaces
list = [
    { name = "Development", layout = "tiled" },
    { name = "Web", layout = "floating" },
    { name = "Terminal", layout = "tiled" }
]

# Default keybindings
[keybindings]
# Mod key (super/ windows key)
mod_key = "Super"

# Application launcher
launch_terminal = "Mod+Return"
launch_launcher = "Mod+D"

# Window management
close_window = "Mod+Q"
toggle_fullscreen = "Mod+F"

# Workspace switching
switch_workspace_1 = "Mod+1"
switch_workspace_2 = "Mod+2"
switch_workspace_3 = "Mod+3"

# Move windows
move_to_workspace_1 = "Mod+Shift+1"
move_to_workspace_2 = "Mod+Shift+2"
move_to_workspace_3 = "Mod+Shift+3"
EOF

    log_success "Default configuration created at $SLATE_CONFIG_DIR/slate.toml"
}

# Verify installation
verify_installation() {
    log_info "Verifying installation..."

    if [ ! -f "$SLATE_INSTALL_DIR/target/release/cli" ]; then
        log_error "Build failed - binary not found"
        exit 1
    fi

    if [ ! -x "$SLATE_BIN_DIR/slate-de" ]; then
        log_error "Executable not found in PATH"
        exit 1
    fi

    log_success "Installation verified successfully ✓"
}

# Print success message and next steps
print_success() {
    echo ""
echo -e "${GREEN}╔════════════════════════════════════════════════════════════╗${NC}"
echo -e "${GREEN}║                                                            ║${NC}"
echo -e "${GREEN}║          Slate-DE Installation Complete!                   ║${NC}"
echo -e "${GREEN}║                                                            ║${NC}"
echo -e "${GREEN}╚════════════════════════════════════════════════════════════╝${NC}"
    echo ""
    echo -e "${BLUE}Installation Details:${NC}"
    echo "  • Slate-DE installed to: $SLATE_INSTALL_DIR"
    echo "  • Binary location: $SLATE_BIN_DIR/slate-de"
    echo "  • Configuration: $SLATE_CONFIG_DIR/slate.toml"
    echo ""
    echo -e "${BLUE}Quick Start:${NC}"
    echo "  1. Restart your terminal or run: source ~/.bashrc"
    echo "  2. Start Slate-DE from a TTY: slate-session"
    echo "  3. Or run directly: slate-de"
    echo ""
    echo -e "${BLUE}Documentation:${NC}"
    echo "  • GitHub: https://github.com/flessan/slate-de"
    echo "  • Configuration: ~/.config/slate/slate.toml"
    echo ""
    echo -e "${YELLOW}Note: Slate-DE is a Wayland compositor.${NC}"
    echo -e "${YELLOW}      Exit your current desktop environment and launch from TTY.${NC}"
    echo ""
}

# Main installation flow
main() {
    echo ""
    echo -e "${BLUE}╔════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${BLUE}║          Welcome to Slate-DE Installer v${SLATE_VERSION}              ║${NC}"
    echo -e "${BLUE}║     A CLI-first, pane-based, Wayland-native DE             ║${NC}"
    echo -e "${BLUE}╚════════════════════════════════════════════════════════════╝${NC}"
    echo ""

    # Run installation steps
    check_os
    detect_distro
    install_dependencies
    install_rust
    setup_directories
    setup_repository
    build_slate
    setup_executables
    create_default_config
    verify_installation
    print_success
}

# Run main function
main "$@"
