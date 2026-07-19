//! `cli` — the primary CLI entry point for Slate-DE.
//!
//! This binary drives the interactive desktop (`slate`) or runs headless
//! commands. It is intentionally small: all real work lives in
//! `slate-shell` and `slate-command`.

use std::process::{self, Command};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    // Launch the interactive desktop when called with no args or `desktop`.
    if args.is_empty() || args[0] == "desktop" || args[0] == "session" {
        // Try the workspace binary `slate` (from `shell` crate) first.
        let slate_bin = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.join("slate")))
            .or_else(|| {
                std::env::var("PATH")
                    .ok()
                    .and_then(|path| {
                        for dir in std::env::split_paths(&path) {
                            let candidate = dir.join("slate");
                            if candidate.exists() {
                                return Some(candidate);
                            }
                        }
                        None
                    })
            })
            .unwrap_or_else(|| std::path::PathBuf::from("slate"));

        println!("Slate-DE — launching interactive desktop environment...");
        println!("  Binary: {}", slate_bin.display());
        println!("  Args:   {:?}\n", args);

        let exit = Command::new(&slate_bin)
            .args(&args[1..])
            .status()
            .unwrap_or_else(|e| {
                eprintln!("Failed to launch desktop: {e}");
                process::exit(1);
            })
            .code()
            .unwrap_or(1);

        process::exit(exit);
    }

    // Headless command mode (`run`, `doctor`, etc.).
    println!("Slate-DE CLI — headless mode");
    println!("Usage: cli <desktop|run ...|doctor ...>");

    // Delegate to the `slate` binary for all other commands.
    let slate_bin = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.join("slate")))
        .unwrap_or_else(|| std::path::PathBuf::from("slate"));

    let mut cmd = Command::new(&slate_bin);
    cmd.args(&args);
    let exit = cmd.status().unwrap_or_else(|e| {
        eprintln!("Failed to run command: {e}");
        process::exit(1);
    });
    process::exit(exit.code().unwrap_or(1));
}
