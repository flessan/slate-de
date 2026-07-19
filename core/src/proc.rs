//! Subprocess helpers.
//!
//! Slate deliberately talks to *existing* system tools (`wpctl`, `nmcli`,
//! `playerctl`, `zbarimg`, …) for system integration, degrading gracefully
//! when they are absent. All of that funnels through this module so behavior
//! (timeouts, capturing, detection) stays uniform.

use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

/// Run a command, capturing stdout and stderr (stdin closed).
pub fn run(cmd: &str, args: &[&str]) -> io::Result<Output> {
    Command::new(cmd)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
}

/// Run a command; on success return trimmed stdout (lossy), else `None`.
pub fn run_trimmed(cmd: &str, args: &[&str]) -> Option<String> {
    let out = run(cmd, args).ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    Some(s)
}

/// Run a shell snippet through `sh -c` for the `run` builtin command.
/// Returns `(success, stdout, stderr)` rendered lossily.
pub fn run_shell(script: &str) -> io::Result<(bool, String, String)> {
    let out = run("sh", &["-c", script])?;
    Ok((
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    ))
}

/// Locate an executable on `PATH`.
pub fn which(prog: &str) -> Option<PathBuf> {
    if prog.contains('/') {
        let p = PathBuf::from(prog);
        return is_executable(&p).then_some(p);
    }
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(prog);
        if is_executable(&candidate) {
            return Some(candidate);
        }
    }
    None
}

/// Whether the executable exists on `PATH`.
pub fn exists(prog: &str) -> bool {
    which(prog).is_some()
}

#[cfg(unix)]
fn is_executable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    p.is_file() && p.metadata().map(|m| m.permissions().mode() & 0o111 != 0).unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(p: &Path) -> bool {
    p.is_file()
}

/// Spawn a detached process (no stdin/stdout/stderr wiring into the desktop).
pub fn spawn_detached(cmd: &str, args: &[&str]) -> io::Result<()> {
    Command::new(cmd)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    Ok(())
}

/// Open a file/URL with the user's default application (`xdg-open` or `gio`).
pub fn open_target(target: &str) -> io::Result<()> {
    if let Some(opener) = which("xdg-open") {
        return spawn_detached(&opener.to_string_lossy(), &[target]);
    }
    if which("gio").is_some() {
        return spawn_detached("gio", &["open", target]);
    }
    Err(io::Error::new(io::ErrorKind::NotFound, "neither xdg-open nor gio found"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_captures_output() {
        let (ok, out, err) = run_shell("printf hello").unwrap();
        assert!(ok);
        assert_eq!(out, "hello");
        assert!(err.is_empty());
    }

    #[test]
    fn run_shell_reports_failure() {
        let (ok, _, err) = run_shell("echo oops 1>&2; exit 3").unwrap();
        assert!(!ok);
        assert!(err.contains("oops"));
    }

    #[test]
    fn trimmed_helper() {
        assert_eq!(run_trimmed("sh", &["-c", "echo hi"]).as_deref(), Some("hi"));
        assert_eq!(run_trimmed("sh", &["-c", "exit 1"]), None);
        assert_eq!(run_trimmed("definitely-not-a-real-binary-xyz", &[]), None);
    }

    #[test]
    fn which_finds_sh() {
        assert!(exists("sh"));
        assert!(!exists("definitely-not-a-real-binary-xyz"));
        // Absolute paths are supported too.
        let sh = which("sh").unwrap();
        assert!(which(sh.to_str().unwrap()).is_some());
    }
}
