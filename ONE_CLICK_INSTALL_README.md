# One-Click Install for Slate-DE

## For End Users

### Install Slate-DE with a single command:

```bash
curl -sSf https://raw.githubusercontent.com/flessan/slate-de/main/install.sh | bash
```

That's it! The installer will:
- Detect your Linux distribution
- Install all dependencies automatically
- Install Rust if needed
- Download and build Slate-DE
- Set up everything for immediate use

**Time to install:** ~5-10 minutes

---

## What Happens During Installation

1. **System Check** - Verifies you're running Linux
2. **Distribution Detection** - Detects Ubuntu, Debian, Arch, Fedora, etc.
3. **Dependency Installation** - Installs build tools, Wayland libraries, etc.
4. **Rust Installation** - Installs Rust if not present
5. **Download** - Clones Slate-DE to `~/.local/share/slate-de`
6. **Build** - Compiles Slate-DE (release mode)
7. **Setup** - Creates symlinks and configuration
8. **Verify** - Ensures everything works

---

## After Installation

### Start Slate-DE:

**From TTY (recommended for full Wayland session):**
```bash
slate-session
```

**From current session (for testing):**
```bash
slate-de
```

### Configuration:

Edit `~/.config/slate/slate.toml` to customize your experience.

---

## Uninstall

Remove Slate-DE completely:

```bash
curl -sSf https://raw.githubusercontent.com/flessan/slate-de/main/scripts/uninstall.sh | bash
```

---

## System Requirements

- **OS:** Linux (any modern distribution)
- **RAM:** 4GB minimum, 8GB recommended
- **Graphics:** GPU with Mesa support
- **Dependencies:** Automatically installed by the script

---

## Supported Distributions

| Distribution  | Status            |
| ------------- | ----------------- |
| Ubuntu 22.04+ | ✅ Fully supported |
| Debian 12+    | ✅ Fully supported |
| Arch Linux    | ✅ Fully supported |
| Fedora 38+    | ✅ Fully supported |
| Pop!_OS       | ✅ Fully supported |
| Others        | ⚠️ Should work     |

---

## Troubleshooting

### "command not found: slate-de"

Add `~/.local/bin` to your PATH:
```bash
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.bashrc
source ~/.bashrc
```

### Build fails

Update Rust and retry:
```bash
rustup update
curl -sSf https://raw.githubusercontent.com/flessan/slate-de/main/install.sh | bash
```

### Other issues

Open an issue on GitHub: https://github.com/flessan/slate-de/issues

---

## For Developers

### Test the installer locally:

```bash
# Test syntax
bash -n scripts/install.sh

# Run test suite
./scripts/test-install.sh

# Install locally
./scripts/install.sh
```

### Modify the installer:

Edit `scripts/install.sh` - it's well-commented and modular.

### Release new version:

```bash
./scripts/release.sh 0.2.0
```

---

## Files Created

### Installer Scripts:
- `install.sh` - Bootstrap script (curl this)
- `scripts/install.sh` - Main installer
- `scripts/uninstall.sh` - Uninstaller
- `scripts/test-install.sh` - Test suite
- `scripts/release.sh` - Release prep

### Documentation:
- `INSTALLATION.md` - Detailed install guide
- `INSTALLER_DEMO.md` - Demo of user experience
- `INSTALL_SUMMARY.md` - Technical summary

### Build System:
- `Makefile` - Convenient make targets

---

## Quick Command Reference

```bash
# Install
curl -sSf https://raw.githubusercontent.com/flessan/slate-de/main/install.sh | bash

# Update (re-run installer)
curl -sSf https://raw.githubusercontent.com/flessan/slate-de/main/install.sh | bash

# Uninstall
curl -sSf https://raw.githubusercontent.com/flessan/slate-de/main/scripts/uninstall.sh | bash

# Run
slate-session

# Configure
nano ~/.config/slate/slate.toml
```

---

## Get Help

- **GitHub:** https://github.com/flessan/slate-de
- **Issues:** https://github.com/flessan/slate-de/issues
- **Documentation:** See `docs/` directory

---

**Enjoy your one-click Slate-DE installation!** 🎉
