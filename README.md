# Slate-DE

Slate-DE is a CLI-first, pane-based, Wayland-native Desktop Environment. It is architected for power users who demand high scriptability, modularity, and a unified command interface for all system interactions.

## Core Philosophy

1. **Everything is a Command**: Every system action, from window management to vision analysis, is exposed via a unified command registry.
2. **Everything is a Pane**: The layout engine treats all surfaces (terminals, applications, vision overlays) as first-class panes within a hierarchical node tree.
3. **Everything is Searchable**: A global search and action overlay allows for fuzzy-finding commands, panes, and vision-indexed text.
4. **Everything is Scriptable**: Deep integration with WASM-based plugins allows for low-latency, cross-language extensibility.

## Architecture

Slate-DE is built as a Rust workspace with clear domain boundaries:

*   **core**: Reactive state management, layout engine, and trait definitions.
*   **compositor**: Smithay-based Wayland compositor logic, handling seats, surfaces, and outputs.
*   **renderer**: High-performance TUI-to-SHM rendering pipeline using Ratatui.
*   **vision**: Modular framework for real-time OCR and computer vision on the desktop.
*   **plugins**: WASM runtime (Wasmtime) for sandboxed extensions.
*   **cli**: The primary entry point and command-line interface.

## System Requirements

*   **OS**: Linux (Wayland-native).
*   **Toolchain**: Rust 1.75+ (Stable).
*   **Dependencies**: 
    *   libwayland-dev
    *   libxkbcommon-dev
    *   libgbm-dev
    *   libinput-dev
    *   libudev-dev
    *   clang/llvm (for Smithay/Bindgen)

## Installation

### One-Click Installation (Recommended)

Slate-DE provides a complete one-click installer that automatically handles dependencies, builds the project, and sets up your environment:

```bash
curl -sSf https://raw.githubusercontent.com/flessan/slate-de/main/install.sh | bash
```

That's it! The installer will:
- Detect your Linux distribution (Ubuntu/Debian/Arch/Fedora)
- Install all required system dependencies
- Install Rust if not present
- Clone the repository to `~/.local/share/slate-de`
- Build Slate-DE in release mode
- Create symlinks in `~/.local/bin`
- Generate default configuration at `~/.config/slate/slate.toml`
- Set up a session launcher (`slate-session`)

After installation, restart your terminal and run:
```bash
slate-session  # Launch from TTY for full Wayland session
# or
slate-de       # Run the CLI directly
```

### Manual Installation

If you prefer to install manually:

1.  **Clone the repository**:
    ```bash
    git clone https://github.com/flessan/slate-de.git
    cd slate-de
    ```

2.  **Install system dependencies**:
    Refer to your distribution's package manager for `wayland`, `libinput`, and `mesa`.

3.  **Build the workspace**:
    ```bash
    cargo build --release
    ```

4.  **Run Slate-DE**:
    ```bash
    ./target/release/cli
    ```

## Configuration

Configuration is managed via TOML located at `~/.config/slate/slate.toml`.

```toml
[server]
socket_name = "slate-0"

[theme]
primary_color = "#88C0D0"
background_color = "#2E3440"

[[workspaces.list]]
name = "Development"
layout = "tiled"
default_panes = [
    { type = "terminal", command = "nvim" },
    { type = "terminal", command = "cargo watch" }
]
```

## Development

### Running Tests
```bash
cargo test --workspace
```

### Building WASM Plugins
```bash
./scripts/build-wasm-plugins.sh
```

## License

This project is licensed under either the MIT License or the Apache License (Version 2.0) IDK WHAT TO CHOOSE PLZ.
