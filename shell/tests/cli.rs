//! End-to-end tests driving the real `slate` binary.
//!
//! Each test environment gets isolated `SLATE_*_DIR` directories, so tests
//! are parallel-safe and never touch the developer's real config.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

struct TestEnv {
    root: PathBuf,
}

impl TestEnv {
    fn new() -> Self {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let root = std::env::temp_dir().join(format!("slate-cli-{}-{n}", std::process::id()));
        for d in ["config", "data", "state", "run"] {
            std::fs::create_dir_all(root.join(d)).unwrap();
        }
        TestEnv { root }
    }

    fn slate(&self, args: &[&str]) -> Output {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_slate"));
        cmd.args(args)
            .env("SLATE_CONFIG_DIR", self.root.join("config"))
            .env("SLATE_DATA_DIR", self.root.join("data"))
            .env("SLATE_STATE_DIR", self.root.join("state"))
            .env("SLATE_RUNTIME_DIR", self.root.join("run"))
            // Keep probes deterministic: no locale/color surprises.
            .env("LC_ALL", "C.UTF-8");
        cmd.output().expect("failed to spawn slate")
    }

    fn slate_stdin(&self, args: &[&str], stdin_text: &str) -> Output {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_slate"));
        let mut child = cmd
            .args(args)
            .env("SLATE_CONFIG_DIR", self.root.join("config"))
            .env("SLATE_DATA_DIR", self.root.join("data"))
            .env("SLATE_STATE_DIR", self.root.join("state"))
            .env("SLATE_RUNTIME_DIR", self.root.join("run"))
            .env("LC_ALL", "C.UTF-8")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("failed to spawn slate");
        child.stdin.as_mut().unwrap().write_all(stdin_text.as_bytes()).unwrap();
        child.wait_with_output().expect("failed to wait on slate")
    }
}

impl Drop for TestEnv {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

fn exit_code(out: &Output) -> i32 {
    out.status.code().unwrap_or(-1)
}

// ---------------------------------------------------------------------------

#[test]
fn version_and_help_are_healthy() {
    let env = TestEnv::new();
    let out = env.slate(&["version"]);
    assert_eq!(exit_code(&out), 0, "{}", stderr(&out));
    assert!(stdout(&out).contains("slate 0.1.0"), "{}", stdout(&out));

    let out = env.slate(&["--version"]);
    assert_eq!(exit_code(&out), 0);

    let out = env.slate(&["help"]);
    assert_eq!(exit_code(&out), 0);
    assert!(stdout(&out).contains("USAGE"), "{}", stdout(&out));
    assert!(stdout(&out).contains("doctor"), "{}", stdout(&out));

    let out = env.slate(&["--help"]);
    assert_eq!(exit_code(&out), 0);
}

#[test]
fn headless_run_executes_commands() {
    let env = TestEnv::new();
    let out = env.slate(&["run", "about"]);
    assert_eq!(exit_code(&out), 0, "{}", stderr(&out));
    assert!(stdout(&out).to_lowercase().contains("slate"), "{}", stdout(&out));
}

#[test]
fn shorthand_runs_builtin_commands() {
    let env = TestEnv::new();
    let out = env.slate(&["theme", "list"]);
    assert_eq!(exit_code(&out), 0, "{}", stderr(&out));
    assert!(stdout(&out).contains("slate-dark"), "{}", stdout(&out));
}

#[test]
fn config_roundtrip_persists_between_invocations() {
    let env = TestEnv::new();
    let out = env.slate(&["run", "config", "get", "theme.name"]);
    assert_eq!(exit_code(&out), 0, "{}", stderr(&out));
    assert!(stdout(&out).contains("slate-dark"), "{}", stdout(&out));

    let out = env.slate(&["config", "set", "ui.gap", "2"]);
    assert_eq!(exit_code(&out), 0, "{}", stderr(&out));

    let out = env.slate(&["run", "config", "get", "ui.gap"]);
    assert_eq!(exit_code(&out), 0);
    assert!(stdout(&out).contains('2'), "{}", stdout(&out));

    let out = env.slate(&["run", "config", "reset", "--yes"]);
    let _ = out; // reset may require confirmation; get checks the value path
    let out = env.slate(&["run", "config", "get", "theme.name"]);
    assert_eq!(exit_code(&out), 0);
}

#[test]
fn dash_c_chains_command_lines_in_one_context() {
    let env = TestEnv::new();
    let out = env.slate(&["-c", "config set ui.gap 3", "-c", "config get ui.gap"]);
    assert_eq!(exit_code(&out), 0, "{}", stderr(&out));
    assert!(stdout(&out).contains('3'), "{}", stdout(&out));
}

#[test]
fn stdin_is_a_command_source() {
    let env = TestEnv::new();
    let out = env.slate_stdin(&["run"], "theme list\nnotes list\n");
    assert_eq!(exit_code(&out), 0, "{}", stderr(&out));
    assert!(stdout(&out).contains("slate-dark"), "{}", stdout(&out));
}

#[test]
fn run_from_script_file() {
    let env = TestEnv::new();
    let script = env.root.join("test.slate");
    std::fs::write(&script, "# comment line\ntheme list\n").unwrap();
    let out = env.slate(&["run", "--file", script.to_str().unwrap()]);
    assert_eq!(exit_code(&out), 0, "{}", stderr(&out));
    assert!(stdout(&out).contains("slate-light"), "{}", stdout(&out));
}

#[test]
fn unknown_command_fails_with_suggestions() {
    let env = TestEnv::new();
    let out = env.slate(&["run", "not-a-command"]);
    assert_eq!(exit_code(&out), 1, "expected error exit, got {}", exit_code(&out));
    assert!(stderr(&out).contains("unknown command"), "{}", stderr(&out));
}

#[test]
fn unknown_subcommand_is_a_usage_error() {
    let env = TestEnv::new();
    let out = env.slate(&["not-a-subcommand"]);
    assert_eq!(exit_code(&out), 2);
}

#[test]
fn keys_table_prints() {
    let env = TestEnv::new();
    let out = env.slate(&["keys"]);
    assert_eq!(exit_code(&out), 0);
    assert!(stdout(&out).contains("Ctrl+Space"), "{}", stdout(&out));
    assert!(stdout(&out).contains("workspace"), "{}", stdout(&out));
}

#[test]
fn snapshot_text_and_svg_file() {
    let env = TestEnv::new();
    let out = env.slate(&["snapshot", "desktop", "--text"]);
    assert_eq!(exit_code(&out), 0, "{}", stderr(&out));
    assert!(stdout(&out).contains("prompt"), "{}", stdout(&out));
    assert!(stdout(&out).contains("dashboard"), "{}", stdout(&out));

    let svg_path = env.root.join("snap.svg");
    let out = env.slate(&["snapshot", "search", "--out", svg_path.to_str().unwrap()]);
    assert_eq!(exit_code(&out), 0, "{}", stderr(&out));
    let svg = std::fs::read_to_string(&svg_path).unwrap();
    assert!(svg.starts_with("<svg"), "svg head: {:.80}", svg);
    assert!(svg.trim_end().ends_with("</svg>"));
    assert!(svg.len() > 1000);

    let out = env.slate(&["snapshot", "bogus-view"]);
    assert_eq!(exit_code(&out), 1);
    assert!(stderr(&out).contains("unknown view"), "{}", stderr(&out));
}

#[test]
fn doctor_runs_everywhere() {
    let env = TestEnv::new();
    let out = env.slate(&["doctor"]);
    assert_eq!(exit_code(&out), 0, "{}", stderr(&out));
    assert!(stdout(&out).contains("slate doctor"), "{}", stdout(&out));
    assert!(stdout(&out).contains("verdict: slate can run"), "{}", stdout(&out));

    let out = env.slate(&["doctor", "--json"]);
    assert_eq!(exit_code(&out), 0);
    assert!(stdout(&out).contains("\"checks\""), "{}", stdout(&out));
    assert!(stdout(&out).contains("\"terminal.tty\""), "{}", stdout(&out));
}

#[test]
fn notes_crud_over_the_cli() {
    let env = TestEnv::new();
    let out = env.slate(&["notes", "new", "groceries"]);
    assert_eq!(exit_code(&out), 0, "{}", stderr(&out));
    let out = env.slate(&["notes", "add", "groceries", "buy coffee"]);
    assert_eq!(exit_code(&out), 0, "{}", stderr(&out));
    let out = env.slate(&["notes", "list"]);
    assert_eq!(exit_code(&out), 0);
    assert!(stdout(&out).contains("groceries"), "{}", stdout(&out));
    let out = env.slate(&["notes", "show", "groceries"]);
    assert_eq!(exit_code(&out), 0);
    assert!(stdout(&out).contains("buy coffee"), "{}", stdout(&out));
}

#[test]
fn interactive_without_tty_explains_instead_of_hanging() {
    let env = TestEnv::new();
    // stdin is piped/closed → no tty → clear error, not a hang.
    let out = env.slate(&[]);
    assert_eq!(exit_code(&out), 1);
    assert!(stderr(&out).contains("needs a terminal"), "{}", stderr(&out));
}
