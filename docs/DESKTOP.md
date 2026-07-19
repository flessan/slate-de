# Slate-DE — Real Desktop Environment Guide

Slate-DE is now a functional, CLI-first, pane-based Wayland-native desktop environment.

## What Changed

- Fixed `smithay` dependency (`desktop` feature removed; correct backend features added)
- Complete workspace members (`shell`, `term`, `theme`, `notes`, `dashboard`, `launcher`, `vision`, etc.)
- Working `cli` launcher
- Real `slate-session` Wayland session launcher
- Installation script updated for working builds

## Quick Start

```bash
# From a TTY (recommended)
slate-session

# Or run directly
slate-de

# Or run the binary directly
./target/release/slate
```

## Key Features

- **Pane-based layout**: Every surface (terminal, notes, dashboard, logs) is a first-class pane
- **Universal search**: `Ctrl+Space` opens a fuzzy search over commands, notes, files, plugins, themes
- **Command-driven**: Everything is a command (`workspace switch`, `notes add`, `theme set`, `vision capture`)
- **WASM plugins**: Sandboxed extensions (`plugins` crate with Wasmtime)
- **Integrated vision**: OCR (`tesseract`), QR (`zbarimg`), visual search (`curl` to providers)
- **Dashboard**: System widgets (clock, date, CPU, memory, disk, battery, network, volume, git, music)
- **Notes**: Persistent Markdown notebooks with pinning, tags, and search indexing
- **Snapshots**: Off-screen deterministic rendering (`slate snapshot desktop --size 120x40`)

## Architecture

- `shell` (`slate`) — interactive desktop event loop, pane management, rendering
- `command` (`slate-command`) — command registry, lexer, built-in commands
- `core` — layout DSL, style, workspace model, effects
- `compositor` — Smithay-based Wayland compositor (seats, surfaces, outputs, SHM)
- `renderer` — SHM-based Ratatui renderer
- `term` — first-party terminal engine (Buffer, Backend, widgets)
- `vision` — pluggable vision providers (OCR, QR, translate, web search)
- `plugins` — WASM runtime for sandboxed extensions

## Installation

```bash
curl -sSf https://raw.githubusercontent.com/flessan/slate-de/main/install.sh | bash
```

This installs:
- System dependencies (`build-essential`, `pkg-config`, Wayland libraries, `clang`, etc.)
- Rust toolchain (if missing)
- Repository to `~/.local/share/slate-de`
- Binary to `~/.local/bin/slate`
- Default config at `~/.config/slate/slate.toml`
- Session launcher `slate-session`
