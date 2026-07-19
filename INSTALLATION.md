# Slate-DE Installation Guide

## Quick Start (One-Click Install)

The easiest way to install Slate-DE is using our one-click installer:

```bash
curl -sSf https://raw.githubusercontent.com/flessan/slate-de/main/install.sh | bash
```

This single command will:
- Detect your Linux distribution
- Install all required dependencies
- Install Rust (if needed)
- Download and build Slate-DE
- Set up configuration files
- Create convenient symlinks

**Time to complete:** ~5-10 minutes (depending on your system)

---

## What Gets Installed

After running the installer, you'll have:

```
~/.local/share/slate-de/     # Main installation directory
    ├── target/release/cli   # The main binary
    ├── core/                 # Core library source
    ├── compositor/          # Compositor source
    └── ...                  # Other workspace members

~/.local/bin/
    ├── slate-de             # Main executable (symlink)
    ├── slate                # Short alias (symlink)
    └── slate-session        # Session launcher script

~/.config/slate/
    └── slate.toml           # Configuration file
```

---

## Installation Methods

### Method 1: One-Click Install (Recommended)

```bash
curl -sSf https://raw.githubusercontent.com/flessan/slate-de/main/install.sh | bash
```

**Perfect for:**
- First-time users
- Quick evaluation
- Automated deployments

### Method 2: Clone and Build

```bash
# Clone the repository
git clone https://github.com/flessan/slate-de.git
cd slate-de

# Install dependencies (Ubuntu/Debian)
sudo apt install -y build-essential pkg-config libwayland-dev \
    libxkbcommon-dev libdbus-1-dev libgbm-dev libinput-dev \
    libudev-dev clang libclang-dev cmake

# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Build
cargo build --release

# Run
./target/release/cli
```

**Perfect for:**
- Developers
- Custom modifications
- Understanding the build process

### Method 3: Using Make

```bash
git clone https://github.com/flessan/slate-de.git
cd slate-de
make quick-install
```

---

## Post-Installation

### 1. Restart Your Shell

```bash
source ~/.bashrc
# or
exec $SHELL
```

### 2. Verify Installation

```bash
which slate-de
slate-de --version
```

### 3. Launch Slate-DE

**From TTY (recommended for full Wayland session):**
```bash
slate-session
```

**From current session (nested mode - for testing):**
```bash
slate-de
```

### 4. Configure

Edit the configuration file:
```bash
nano ~/.config/slate/slate.toml
```

See [Configuration](#configuration) section below.

---

## System Requirements

### Minimum Requirements

- **OS:** Linux (any modern distribution)
- **RAM:** 4GB
- **Graphics:** GPU with Mesa support
- **Display:** Monitor capable of at least 1280x720

### Recommended Requirements

- **OS:** Ubuntu 22.04+ / Arch Linux / Fedora 38+
- **RAM:** 8GB+
- **Graphics:** Dedicated GPU with Mesa 23.0+
- **Display:** 1920x1080+ monitor

### Supported Distributions

| Distribution        | Status            | Notes                 |
| ------------------- | ----------------- | --------------------- |
| Ubuntu 22.04+       | ✅ Fully supported | Tested                |
| Debian 12+          | ✅ Fully supported | Tested                |
| Arch Linux          | ✅ Fully supported | Tested                |
| Fedora 38+          | ✅ Fully supported | Tested                |
| Pop!_OS             | ✅ Fully supported | Tested                |
| openSUSE            | ⚠️ Should work     | Untested              |
| Other distributions | ⚠️ Manual install  | Dependencies may vary |

---

## Dependencies

The installer automatically installs these dependencies:

### Ubuntu/Debian
```
build-essential pkg-config libwayland-dev libxkbcommon-dev
libdbus-1-dev libgbm-dev libinput-dev libudev-dev clang
libclang-dev cmake git curl rustup
```

### Arch Linux
```
base-devel wayland libxkbcommon dbus mesa libinput
clang cmake git curl rustup
```

### Fedora
```
gcc gcc-c++ make pkg-config wayland-devel libxkbcommon-devel
dbus-devel mesa-libgbm-devel libinput-devel systemd-devel
clang cmake git curl rustup
```

---

## Configuration

### Default Configuration Location

```
~/.config/slate/slate.toml
```

### Example Configuration

```toml
[server]
socket_name = "slate-0"

[theme]
primary_color = "#88C0D0"
background_color = "#2E3440"
text_color = "#D8DEE9"

[compositor]
vsync = true

[workspaces]
list = [
    { name = "Development", layout = "tiled" },
    { name = "Web", layout = "floating" },
    { name = "Terminal", layout = "tiled" }
]

[keybindings]
mod_key = "Super"
launch_terminal = "Mod+Return"
launch_launcher = "Mod+D"
close_window = "Mod+Q"
```

See `config/` directory for more configuration options.

---

## Troubleshooting

### Installer Fails

**Problem:** Installer stops with an error.

**Solution:**
1. Check your internet connection
2. Ensure you have `sudo` privileges
3. Try manual installation
4. Open an issue on GitHub with the error message

### Binary Not Found

**Problem:** `slate-de: command not found`

**Solution:**
```bash
# Add to PATH
export PATH="$HOME/.local/bin:$PATH"

# Make permanent
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.bashrc
```

### Build Fails

**Problem:** `cargo build` fails with compilation errors.

**Solution:**
1. Update Rust: `rustup update`
2. Clean build: `cargo clean && cargo build --release`
3. Check Rust version: `rustc --version` (need 1.75+)

### Wayland Session Won't Start

**Problem:** `slate-session` fails to launch.

**Solution:**
1. Ensure you're running from a TTY (Ctrl+Alt+F3)
2. Check GPU drivers are installed
3. Review logs: `~/.local/share/slate-de/logs/`

---

## Uninstalling

To completely remove Slate-DE:

```bash
curl -sSf https://raw.githubusercontent.com/flessan/slate-de/main/scripts/uninstall.sh | bash
```

Or manually:
```bash
rm -rf ~/.local/share/slate-de
rm -f ~/.local/bin/slate-de
rm -f ~/.local/bin/slate
rm -f ~/.local/bin/slate-session
rm -rf ~/.config/slate
```

---

## Getting Help

- **GitHub Issues:** https://github.com/flessan/slate-de/issues
- **Documentation:** See `docs/` directory
- **Examples:** See `config/` directory

---

## Next Steps

After installation:
1. Read the [README.md](../README.md) for project overview
2. Check out the [docs/](docs/) directory for detailed documentation
3. Join the community and share your experience!

---

**Happy Slating!** 
