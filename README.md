<div align="center">

<h1>Slate-DE</h1>
<sub>
    Slate-DE is a CLI-first, pane-based, Wayland-native Desktop Environment. It is architected for power users who demand high scriptability, modularity, and a unified command interface for all system interactions.
</sub>

<br>
<br>

<div style="border-radius: 23px; overflow: hidden; width: 516px;">
  <img width="516" alt="slate-de" src="https://github.com/user-attachments/assets/b906c44c-31ba-4ec2-83c1-fdb09a6c3c7a" style="max-width: 100%; display: block;" />
</div>


---

| **Current Version** | **Build Status** | **Branch** | **Rust Version** |
| :--- | :--- | :--- | :--- |
| `v0.1.0-alpha` | `passing` | `main` | `1.75+ (Stable)` |

</div>

---

> [!NOTE]
> This project is currently in active development. We are breaking things and putting them back together daily, so expect a little bit of chaos as we march toward stability.


Hello world. Slate-DE is a tiny but mighty CLI-first, pane-based, Wayland-native Desktop Environment. We made it for power users who love heavy scripting, clean modular pieces, and having just one cozy command interface to control everything.

## The Ideas We Love

- [x] **Everything is a Command** - No hidden magic here. Every single system action, from moving your windows around to doing clever vision analysis, lives inside one friendly command registry.
- [x] **Everything is a Pane** - Our layout engine treats every surface (your terminal, regular apps, or even vision overlays) as equals in a big, happy node tree.
- [x] **Everything is Searchable** - Need to find something quickly? The global search and action overlay lets you fuzzy-find your commands, open panes, and even text indexed by the vision system.
- [x] **Everything is Scriptable** - We hooked things up deeply with WASM-based plugins, meaning you can write extensions in your favorite languages with super low latency.

## Under the Hood

Slate-DE is split into neat little Rust workspaces that keep our codebase happy and organized:

```diff
+ [Workspace Root]
  ├── 📂 core        --> Reactive state management & layout engine
  ├── 📂 compositor  --> Smithay-based Wayland compositor logic
  ├── 📂 renderer    --> High-performance TUI-to-SHM pipeline via Ratatui
  ├── 📂 vision      --> Real-time OCR & computer vision framework
  ├── 📂 plugins     --> Sandboxed WASM runtime using Wasmtime
  └── 📂 cli         --> Cozy front door and primary command line
```

<details>
<summary><b>🔍 Click here to peek at the detailed system requirements</b></summary>

### Things You Will Need

*   **OS**: Any Linux setup running native Wayland.
*   **Toolchain**: Rust 1.75 or newer (stable channel is perfect).
*   **System Libraries**: 
    *   `libwayland-dev`
    *   `libxkbcommon-dev`
    *   `libgbm-dev`
    *   `libinput-dev`
    *   `libudev-dev`
    *   `clang/llvm` *(so Smithay and Bindgen can talk nicely)*
</details>

## Getting Started

### The Quick Setup (Highly Recommended)

We made a handy installer script that does all the heavy lifting for you. It grabs the dependencies, builds the code, and sets up your environment automatically:

```bash
curl -sSf https://raw.githubusercontent.com/flessan/slate-de/main/install.sh | bash
```

> [!TIP]
> **What the script actually does behind your back:**
> - Guesses your Linux distribution (Ubuntu, Debian, Arch, or Fedora)
> - Grabs all the required system packages
> - Helps install Rust if you do not have it yet
> - Clones the repository safely into `~/.local/share/slate-de`
> - Compiles Slate-DE in release mode (maximum speed)
> - Drops neat symlinks into `~/.local/bin`
> - Spawns a default config file at `~/.config/slate/slate.toml`
> - Prepares a session launcher called `slate-session`

Once it is finished, just restart your terminal and type:
```bash
slate-session  # Run this from a TTY to kick off a full Wayland session
# or
slate-de       # If you just want to play with the CLI directly
```

### Doing it by Hand

If you prefer building things yourself step-by-step:

```bash
# 1. Grab the code
git clone https://github.com/flessan/slate-de.git && cd slate-de

# 2. Build everything for maximum speed
cargo build --release

# 3. Take it for a spin!
./target/release/cli
```

## Tweaking Your Config

You can make Slate-DE feel like home by editing the TOML file at `~/.config/slate/slate.toml`. 

```toml
[server]
socket_name = "slate-0"

[theme]
primary_color = "#88C0D0"      # A lovely frosty blue
background_color = "#2E3440"   # Deep slate background

[[workspaces.list]]
name = "Development"
layout = "tiled"
default_panes = [
    { type = "terminal", command = "nvim" },
    { type = "terminal", command = "cargo watch" }
]
```

## Contributing and Coding

We love code contributions. Here are the two commands you will need the most while hacking on things:

```bash
# To make sure you didn't break anything
cargo test --workspace

# To build the bundled web assembly plugins
./scripts/build-wasm-plugins.sh
```

## License

Choosing licenses can be tricky. We decided to dual-license this project under both the **MIT License** and the **Apache License (Version 2.0)**. This gives everyone the ultimate freedom to use, modify, and share the code however they like.
