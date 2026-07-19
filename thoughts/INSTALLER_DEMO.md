# Slate-DE Installer Demo

This file shows what users will see when they run the one-click installer.

---

## User Experience

### Step 1: User runs the command

```bash
curl -sSf https://raw.githubusercontent.com/flessan/slate-de/main/install.sh | bash
```

### Step 2: Installer starts

```
╔════════════════════════════════════════════════════════════╗
║           Welcome to Slate-DE Installer v0.1.0              ║
║     A CLI-first, pane-based, Wayland-native DE             ║
╚════════════════════════════════════════════════════════════╝

[INFO] Operating System: Linux ✓
[INFO] Detected distribution: Ubuntu 22.04.3 LTS
[INFO] Installing system dependencies...
```

### Step 3: Dependencies install

```
[sudo] password for user:
Reading package lists... Done
Building dependency tree... Done
The following NEW packages will be installed:
  build-essential pkg-config libwayland-dev libxkbcommon-dev
  libdbus-1-dev libgbm-dev libinput-dev libudev-dev clang
  libclang-dev cmake git curl rustup
0 upgraded, 42 newly installed, 0 to remove.
After this operation, 450 MB of additional disk space will be used.
Do you want to continue? [Y/n] Y
...
[SUCCESS] System dependencies installed
```

### Step 4: Rust installation (if needed)

```
[INFO] Rust/Cargo not found. Installing via rustup...
info: downloading installer
info: rustup installer downloaded
info: installing rust toolchain
info: default toolchain set to stable-x86_64-unknown-linux-gnu
[SUCCESS] Rust installed successfully
rustc 1.75.0 (2024-01-01)
```

### Step 5: Directory setup

```
[INFO] Setting up directories...
[SUCCESS] Directories created
```

### Step 6: Clone/Update repository

```
[INFO] Cloning Slate-DE repository...
Cloning into '/home/user/.local/share/slate-de'...
remote: Enumerating objects: 1234, done.
Receiving objects: 100% (1234/1234), 5.67 MiB | 10.00 MiB/s, done.
[SUCCESS] Repository ready
```

### Step 7: Build

```
[INFO] Building Slate-DE (this may take a few minutes)...
   Compiling core v0.1.0 (/home/user/.local/share/slate-de/core)
   Compiling compositor v0.1.0 (/home/user/.local/share/slate-de/compositor)
   Compiling renderer v0.1.0 (/home/user/.local/share/slate-de/renderer)
   ...
   Compiling cli v0.1.0 (/home/user/.local/share/slate-de/cli)
    Finished release [optimized] target(s) in 4m 32s
[SUCCESS] Build complete
```

### Step 8: Setup executables

```
[INFO] Setting up executables...
[SUCCESS] Executables installed to /home/user/.local/bin
```

### Step 9: Create configuration

```
[INFO] Creating default configuration...
[SUCCESS] Default configuration created at /home/user/.config/slate/slate.toml
```

### Step 10: Verification

```
[INFO] Verifying installation...
[SUCCESS] Installation verified successfully ✓
```

### Step 11: Success message

```
╔════════════════════════════════════════════════════════════╗
║                                                            ║
║          Slate-DE Installation Complete! 🎉                ║
║                                                            ║
╚════════════════════════════════════════════════════════════╝

Installation Details:
  • Slate-DE installed to: /home/user/.local/share/slate-de
  • Binary location: /home/user/.local/bin/slate-de
  • Configuration: /home/user/.config/slate/slate.toml

Quick Start:
  1. Restart your terminal or run: source ~/.bashrc
  2. Start Slate-DE from a TTY: slate-session
  3. Or run directly: slate-de

Documentation:
  • GitHub: https://github.com/flessan/slate-de
  • Configuration: ~/.config/slate/slate.toml

Note: Slate-DE is a Wayland compositor.
      Exit your current desktop environment and launch from TTY.
```

---

## Total Time

- **First installation:** ~5-10 minutes
  - Dependencies: ~2-3 minutes
  - Rust install: ~1-2 minutes (if needed)
  - Clone: ~30 seconds
  - Build: ~3-5 minutes

- **Update existing installation:** ~2-3 minutes
  - Pull changes: ~10 seconds
  - Rebuild: ~2-3 minutes

---

## Error Handling

### If dependency installation fails:

```
[ERROR] Failed to install dependencies
Please install these packages manually:
  - build-essential
  - libwayland-dev
  - ...
Then re-run this script.
```

### If Rust installation fails:

```
[ERROR] Rust installation failed
Please install Rust manually: https://rustup.rs
Then re-run this script.
```

### If build fails:

```
[ERROR] Build failed
Check the error messages above.
Common issues:
  - Insufficient disk space
  - Outdated Rust version (need 1.75+)
  - Missing system dependencies

Try running: cargo clean && cargo build --release
Or open an issue: https://github.com/flessan/slate-de/issues
```

---

## Re-running the Installer

The installer is idempotent - safe to run multiple times:

```bash
# First run - fresh install
curl -sSf https://raw.githubusercontent.com/flessan/slate-de/main/install.sh | bash

# Second run - updates to latest version
curl -sSf https://raw.githubusercontent.com/flessan/slate-de/main/install.sh | bash

# Output on second run:
[INFO] Updating existing Slate-DE repository...
Already up to date.
[INFO] Building Slate-DE...
    Finished release [optimized] target(s) in 1m 23s
[SUCCESS] Update complete!
```

---

## Uninstallation

Users can uninstall with:

```bash
curl -sSf https://raw.githubusercontent.com/flessan/slate-de/main/scripts/uninstall.sh | bash
```

This will show:

```
=== Slate-DE Uninstaller ===

This will remove:
  - /home/user/.local/share/slate-de
  - /home/user/.local/bin/slate-de
  - /home/user/.local/bin/slate
  - /home/user/.local/bin/slate-session

Are you sure you want to continue? (y/N) y

Removing Slate-DE...
  ✓ Removed binaries
  ✓ Removed installation directory

Remove configuration directory (/home/user/.config/slate)? (y/N) y
  ✓ Removed configuration

Remove plugin and log data? (y/N) y
  ✓ Removed data directory

=== Uninstallation Complete ===

Slate-DE has been removed from your system.
```

---

## Tips for Best Experience

### For Users
1. Run from a stable internet connection
2. Ensure you have ~2GB free disk space
3. Use a modern distribution (Ubuntu 22.04+, Arch, Fedora 38+)
4. Have sudo password ready

### For Developers
1. Test on fresh VMs before release
2. Keep dependencies list updated
3. Monitor GitHub issues for installation problems
4. Consider providing pre-built binaries for faster install

---

**The installer provides a smooth, professional experience that gets users from "curious" to "running Slate-DE" in under 10 minutes with a single command.**
