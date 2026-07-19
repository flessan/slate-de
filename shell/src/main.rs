//! `slate` — the CLI-first desktop.
//!
//! Invocation surface (all of it ends up in the same command engine):
//!
//! ```text
//! slate                          start the interactive desktop
//! slate run '<line>'             execute one command line headlessly
//! slate -c '<line>' -c '<line>'  execute several command lines headlessly
//! slate run --file script.slate  run a slate script (one command per line)
//! echo 'theme list' | slate run  read command lines from stdin
//! slate <command> [args…]        shorthand for `slate run '<command> …'`
//! slate doctor                   environment self-check
//! slate keys                     keybinding table
//! slate snapshot [view]          deterministic off-screen render (SVG/text)
//! slate version                  version info
//! slate help                     this text
//! ```

#![forbid(unsafe_code)]

use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use slate_command::{Ctx, Registry, KEYBINDINGS};
use slate_core::args::{take_flag, take_option};
use slate_core::error::{Error, Result};
use slate_core::style::Line;
use slate_core::{APP_NAME, VERSION};
use slate_shell::{render_snapshot, App, SnapshotOptions, SnapshotView};
use slate_term::backend::{AnsiBackend, RawMode};
use slate_term::input::KeyDecoder;
use slate_term::Terminal;

/// Conventional exit codes (documented in `docs/DEVELOPER.md`).
const EXIT_OK: u8 = 0;
const EXIT_ERROR: u8 = 1;
const EXIT_USAGE: u8 = 2;

const USAGE: &str = "\
slate — the command line is your desktop

USAGE
  slate                          start the interactive desktop
  slate run '<line>'             execute one command line headlessly
  slate -c '<line>' [-c '<line>']  run several command lines headlessly
  slate run --file <script>      run a .slate script (one command per line)
  slate <command> [args…]        shorthand for `slate run '<command> [args…]'`
  slate doctor                   environment self-check
  slate keys                     print the keybinding table
  slate snapshot [view]          render the desktop off-screen
  slate version                  print version information
  slate help                     print this text

SNAPSHOT OPTIONS
  slate snapshot <desktop|search|notes|ascii|dashboard>
                 [--size WxH] [--text] [--out FILE]

ENVIRONMENT
  SLATE_CONFIG_DIR  config directory    (default ~/.config/slate)
  SLATE_DATA_DIR    data directory      (default ~/.local/share/slate)
  SLATE_STATE_DIR   state directory     (default ~/.local/state/slate)
  SLATE_RUNTIME_DIR control socket dir  (default $XDG_RUNTIME_DIR or /tmp)

FILES
  config.toml, themes/*.toml  see `slate run config` and docs/CONFIGURATION.md
  history, slate.log          under the state directory
  slate.sock                  IPC socket used by `slatectl`

Exit codes: 0 ok · 1 error · 2 usage.
Docs: https://github.com/flessan/slate-de
";

fn main() -> std::process::ExitCode {
    install_panic_hook();
    let args: Vec<String> = std::env::args().skip(1).collect();
    match dispatch(args) {
        Ok(code) => std::process::ExitCode::from(code),
        Err(e) => {
            eprintln!("{APP_NAME}: {e}");
            std::process::ExitCode::from(EXIT_ERROR)
        }
    }
}

fn dispatch(mut args: Vec<String>) -> Result<u8> {
    if args.is_empty() {
        return run_desktop();
    }
    if take_flag(&mut args, &["-h", "--help"]) && args.len() <= 1 {
        print!("{USAGE}");
        return Ok(EXIT_OK);
    }
    if take_flag(&mut args, &["-V", "--version"]) && args.len() <= 1 {
        print_version();
        return Ok(EXIT_OK);
    }
    // `-c '<line>'` may repeat; collect them all.
    let mut lines: Vec<String> = Vec::new();
    while args.first().map(String::as_str) == Some("-c") {
        args.remove(0);
        if args.is_empty() {
            return Err(Error::invalid("'-c' requires a command line argument"));
        }
        lines.push(args.remove(0));
    }
    if !lines.is_empty() {
        let rest = if args.is_empty() { Vec::new() } else { vec![args.join(" ")] };
        lines.extend(rest);
        return run_lines(&lines, false);
    }
    let head = args.remove(0);
    match head.as_str() {
        "run" => cmd_run(args),
        "doctor" => cmd_doctor(&args),
        "keys" => {
            print_keys();
            Ok(EXIT_OK)
        }
        "snapshot" => cmd_snapshot(args),
        "version" => {
            print_version();
            Ok(EXIT_OK)
        }
        "help" => {
            print!("{USAGE}");
            Ok(EXIT_OK)
        }
        name => {
            // Shorthand: any built-in command name runs headlessly.
            if Registry::builtins().find(name).is_some() {
                let mut all = vec![name.to_string()];
                all.extend(args);
                run_lines(&[all.join(" ")], false)
            } else {
                eprintln!("{APP_NAME}: unknown subcommand '{name}'\n");
                print!("{USAGE}");
                Ok(EXIT_USAGE)
            }
        }
    }
}

// --------------------------------------------------------------------------
// headless execution
// --------------------------------------------------------------------------

/// Execute command lines without a UI and print their plain-text output.
fn cmd_run(mut args: Vec<String>) -> Result<u8> {
    let keep_going = take_flag(&mut args, &["-k", "--keep-going"]);
    let file = take_option(&mut args, &["-f", "--file"]);
    let mut lines: Vec<String> = Vec::new();
    if let Some(path) = &file {
        let text = std::fs::read_to_string(path)
            .map_err(|e| Error::unavailable(format!("cannot read {path}: {e}")))?;
        lines.extend(text.lines().map(str::to_string));
    }
    if !args.is_empty() {
        lines.push(args.join(" "));
    }
    if lines.is_empty() {
        if stdin_is_tty() {
            eprintln!("{APP_NAME} run: nothing to do — pass a command line, --file, or pipe stdin\n");
            print!("{USAGE}");
            return Ok(EXIT_USAGE);
        }
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .map_err(|e| Error::unavailable(format!("reading stdin: {e}")))?;
        lines.extend(buf.lines().map(str::to_string));
    }
    run_lines(&lines, keep_going)
}

fn run_lines(lines: &[String], keep_going: bool) -> Result<u8> {
    let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
    match slate_shell::run_headless(&refs, keep_going) {
        Ok(out) => {
            let text = out.to_plain_string();
            if !text.trim().is_empty() {
                println!("{text}");
            }
            Ok(EXIT_OK)
        }
        Err(e) => Err(e),
    }
}

/// True when stdin is a terminal (detected via `stty`, no libc needed).
fn stdin_is_tty() -> bool {
    slate_core::proc::run("stty", &["-g"]).map(|o| o.status.success()).unwrap_or(false)
}

// --------------------------------------------------------------------------
// snapshot
// --------------------------------------------------------------------------

fn cmd_snapshot(mut args: Vec<String>) -> Result<u8> {
    let text_mode = take_flag(&mut args, &["--text"]);
    let out = take_option(&mut args, &["-o", "--out"]);
    let size = take_option(&mut args, &["-s", "--size"]);
    let width = take_option(&mut args, &["-w", "--width"])
        .and_then(|v| v.parse::<u16>().ok());
    let height = take_option(&mut args, &["--height"]).and_then(|v| v.parse::<u16>().ok());
    let (mut w, mut h) = (width.unwrap_or(120), height.unwrap_or(34));
    if let Some(sz) = size {
        let mut it = sz.split(['x', 'X']);
        let pw = it.next().and_then(|v| v.parse::<u16>().ok());
        let ph = it.next().and_then(|v| v.parse::<u16>().ok());
        match (pw, ph) {
            (Some(pw), Some(ph)) => {
                w = pw;
                h = ph;
            }
            _ => return Err(Error::invalid("--size expects WxH, e.g. --size 120x34")),
        }
    }
    // Remaining positional: the view name.
    let view_name = args.first().map(String::as_str).unwrap_or("desktop");
    let view = SnapshotView::parse(view_name).ok_or_else(|| {
        Error::invalid(format!(
            "unknown view '{view_name}' — expected one of: {}",
            SnapshotView::names().join(", ")
        ))
    })?;
    let opts = SnapshotOptions { view, width: w, height: h, text: text_mode };
    let rendered = render_snapshot(&opts)?;
    match out {
        Some(path) => {
            std::fs::write(&path, &rendered)
                .map_err(|e| Error::unavailable(format!("cannot write {path}: {e}")))?;
            eprintln!("{APP_NAME}: wrote {path} ({} bytes)", rendered.len());
        }
        None => print!("{rendered}"),
    }
    Ok(EXIT_OK)
}

// --------------------------------------------------------------------------
// keys / version
// --------------------------------------------------------------------------

fn print_keys() {
    println!("slate keybindings (also available in-app via `keys`)\n");
    let width = KEYBINDINGS.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
    for (keys, desc) in KEYBINDINGS {
        println!("  {keys:<width$}  {desc}");
    }
    println!("\nThe search overlay and panes add their own bindings; see docs/KEYBINDINGS.md.");
}

fn print_version() {
    println!("{APP_NAME} {VERSION} — a CLI-first desktop environment");
    println!("zero-dependency · zero-unsafe · MIT OR Apache-2.0");
    println!("{}", slate_command::DOCS_URL);
}

// --------------------------------------------------------------------------
// doctor
// --------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Status {
    Ok,
    Warn,
    Fail,
    Info,
}

impl Status {
    fn label(self) -> &'static str {
        match self {
            Status::Ok => "ok",
            Status::Warn => "warn",
            Status::Fail => "FAIL",
            Status::Info => "info",
        }
    }
}

struct Check {
    name: String,
    status: Status,
    detail: String,
}

fn cmd_doctor(args: &[String]) -> Result<u8> {
    let json = args.iter().any(|a| a == "--json");
    let checks = gather_checks();
    if json {
        print_doctor_json(&checks);
    } else {
        println!("slate doctor — environment self-check (slate {VERSION})\n");
        let width = checks.iter().map(|c| c.name.len()).max().unwrap_or(0);
        for c in &checks {
            println!("  [{:<4}] {:<width$}  {}", c.status.label(), c.name, c.detail);
        }
        let fails = checks.iter().filter(|c| c.status == Status::Fail).count();
        let warns = checks.iter().filter(|c| c.status == Status::Warn).count();
        println!("\n{} checks, {} warnings, {} failures", checks.len(), warns, fails);
        if fails == 0 {
            println!("verdict: slate can run on this system.");
        } else {
            println!("verdict: fix the FAIL items above (see docs/INSTALL.md#troubleshooting).");
        }
    }
    let code = if checks.iter().any(|c| c.status == Status::Fail) { EXIT_ERROR } else { EXIT_OK };
    Ok(code)
}

fn tool(name: &str, purpose: &str, optional: bool) -> Check {
    match slate_core::proc::which(name) {
        Some(path) => Check {
            name: format!("tool.{name}"),
            status: Status::Ok,
            detail: format!("{} ({})", path.display(), purpose),
        },
        None => Check {
            name: format!("tool.{name}"),
            status: if optional { Status::Warn } else { Status::Fail },
            detail: format!("not found — {purpose}"),
        },
    }
}

fn gather_checks() -> Vec<Check> {
    fn push(checks: &mut Vec<Check>, name: &str, status: Status, detail: String) {
        checks.push(Check { name: name.to_string(), status, detail });
    }

    let mut checks: Vec<Check> = Vec::new();

    // --- terminal -------------------------------------------------------
    push(
        &mut checks,
        "terminal.tty",
        if stdin_is_tty() { Status::Ok } else { Status::Info },
        if stdin_is_tty() {
            "raw mode available (interactive desktop ready)".to_string()
        } else {
            "no tty — use `slate run` / `slatectl` headlessly".to_string()
        },
    );
    let term = std::env::var("TERM").unwrap_or_default();
    push(
        &mut checks,
        "terminal.term",
        if term.is_empty() || term == "dumb" { Status::Warn } else { Status::Ok },
        if term.is_empty() { "TERM is not set".to_string() } else { format!("TERM={term}") },
    );
    let truecolor = matches!(
        std::env::var("COLORTERM").unwrap_or_default().as_str(),
        "truecolor" | "24bit"
    );
    push(
        &mut checks,
        "terminal.colors",
        if truecolor { Status::Ok } else { Status::Warn },
        if truecolor {
            "truecolor (24-bit)".to_string()
        } else {
            "no COLORTERM=truecolor — 256-color fallback in use".to_string()
        },
    );
    let locale = ["LANG", "LC_ALL", "LC_CTYPE"]
        .iter()
        .filter_map(|k| std::env::var(k).ok())
        .find(|v| !v.is_empty())
        .unwrap_or_default();
    let utf8 = locale.to_uppercase().replace('-', "").contains("UTF8");
    push(
        &mut checks,
        "terminal.utf8",
        if utf8 { Status::Ok } else { Status::Warn },
        if utf8 {
            format!("UTF-8 locale ({locale})")
        } else {
            "no UTF-8 locale detected — run `theme set ascii` as a fallback".to_string()
        },
    );

    // --- core tools (required by probes/commands) ------------------------
    checks.push(tool("stty", "terminal size/raw mode", false));
    checks.push(tool("df", "disk widget", false));
    checks.push(tool("date", "clock/status bar", false));

    // --- desktop integrations (optional) ---------------------------------
    checks.push(tool("git", "git widget + `git` command", true));
    checks.push(tool("xdg-open", "opening files/URLs", true));
    checks.push(tool("playerctl", "music command/widget", true));
    let volume_tool = ["wpctl", "pamixer", "amixer"]
        .iter()
        .find(|t| slate_core::proc::exists(t));
    push(
        &mut checks,
        "tool.volume",
        if volume_tool.is_some() { Status::Ok } else { Status::Warn },
        match volume_tool {
            Some(t) => format!("{t} (volume control)"),
            None => "none of wpctl/pamixer/amixer found — `volume` disabled".to_string(),
        },
    );
    checks.push(tool("nmcli", "wifi command", true));
    checks.push(tool("bluetoothctl", "bluetooth command", true));
    checks.push(tool("brightnessctl", "brightness command", true));
    checks.push(tool("curl", "weather command", true));

    // --- vision providers (optional) --------------------------------------
    checks.push(tool("tesseract", "vision ocr", true));
    checks.push(tool("zbarimg", "vision qr", true));
    checks.push(tool("trans", "vision translate", true));

    // --- filesystem ---------------------------------------------------------
    match slate_config::paths::ensure_dirs() {
        Ok(()) => push(
            &mut checks,
            "fs.dirs",
            Status::Ok,
            format!(
                "config={} data={}",
                slate_config::paths::config_dir().display(),
                slate_config::paths::data_dir().display()
            ),
        ),
        Err(e) => push(&mut checks, "fs.dirs", Status::Fail, format!("cannot create slate dirs: {e}")),
    }
    let socket = slate_config::paths::socket_file();
    let runtime_ok = socket.parent().map(|p| p.exists() || std::fs::create_dir_all(p).is_ok());
    push(
        &mut checks,
        "fs.socket",
        if runtime_ok == Some(true) { Status::Ok } else { Status::Fail },
        format!("control socket: {}", socket.display()),
    );

    // --- configuration -------------------------------------------------------
    match slate_config::Config::load() {
        Ok(cfg) => {
            let theme = cfg.get_str("theme.name", "slate-dark");
            let keys = cfg.doc.keys().len();
            push(
                &mut checks,
                "config.parse",
                Status::Ok,
                format!("config.toml valid — {keys} keys, theme={theme}"),
            );
        }
        Err(e) => push(
            &mut checks,
            "config.parse",
            Status::Fail,
            format!("config.toml is broken: {e} (`slate run config reset` to repair)"),
        ),
    }

    // --- session -------------------------------------------------------------
    if socket.exists() && slate_ipc::send(&socket, "about").map(|r| r.ok).unwrap_or(false) {
        push(
            &mut checks,
            "session",
            Status::Info,
            "a slate session is running on the control socket".to_string(),
        );
    } else {
        push(
            &mut checks,
            "session",
            Status::Info,
            "no running slate session (start with `slate`)".to_string(),
        );
    }
    checks
}

fn print_doctor_json(checks: &[Check]) {
    // Hand-rolled JSON: the workspace has no serde by design.
    let mut out = String::from("{\"version\":\"");
    out.push_str(VERSION);
    out.push_str("\",\"checks\":[");
    for (i, c) in checks.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&format!(
            "{{\"name\":\"{}\",\"status\":\"{}\",\"detail\":\"{}\"}}",
            json_escape(&c.name),
            c.status.label(),
            json_escape(&c.detail)
        ));
    }
    out.push_str("]}");
    println!("{out}");
}

fn json_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n")
}

// --------------------------------------------------------------------------
// panic plumbing
// --------------------------------------------------------------------------

static PANIC_MESSAGE: Mutex<Option<String>> = Mutex::new(None);
static HOOK_INSTALLED: AtomicBool = AtomicBool::new(false);

/// Route panic payloads into a side channel so the event loop can surface
/// them in the logs pane instead of smearing them over the alternate screen.
fn install_panic_hook() {
    if HOOK_INSTALLED.swap(true, Ordering::SeqCst) {
        return;
    }
    std::panic::set_hook(Box::new(|info| {
        let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "unknown panic".to_string()
        };
        let where_ = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_else(|| "unknown location".to_string());
        if let Ok(mut slot) = PANIC_MESSAGE.lock() {
            *slot = Some(format!("{payload} ({where_})"));
        }
    }));
}

fn take_panic() -> Option<String> {
    PANIC_MESSAGE.lock().ok().and_then(|mut s| s.take())
}

/// Push a line into the app's logs pane from the driver (App::log is crate-private).
fn driver_log(app: &mut App, msg: impl Into<String>) {
    const MAX_LOGS: usize = 500;
    let style = app.ctx.theme().dim();
    if app.logs.len() >= MAX_LOGS {
        app.logs.pop_front();
    }
    app.logs.push_back(Line::styled(msg.into(), style));
    app.dirty = true;
}

// --------------------------------------------------------------------------
// interactive desktop
// --------------------------------------------------------------------------

/// Emergency escape cleanup: if anything escapes the event loop (a panic
/// outside a catch boundary, teardown I/O failure), the Drop impl still
/// returns the terminal to a sane state. Idempotent by construction.
struct TermGuard;

impl Drop for TermGuard {
    fn drop(&mut self) {
        let mut out = std::io::stdout();
        let _ = out.write_all(b"\x1b[0m\x1b[?25h\x1b[?1049l");
        let _ = out.flush();
    }
}

fn run_desktop() -> Result<u8> {
    let ctx = Ctx::load()?;
    let mut app = App::new(ctx, Registry::builtins());

    // Terminal setup. `_term_guard` is declared first so it drops last.
    let _term_guard = TermGuard;
    let raw = RawMode::enable();
    if !raw.is_active() {
        return Err(Error::unavailable(
            "the interactive desktop needs a terminal.\n\
             hint: in headless contexts use `slate run '<command>'`, `slate snapshot`, \
             or `slate doctor`",
        ));
    }
    let stdout = std::io::stdout();
    let mut backend = AnsiBackend::new(stdout);
    backend.enter_altscreen()?;
    backend.hide_cursor()?;
    backend.flush()?;
    let mut term = Terminal::new(backend);

    // IPC control socket (best-effort; slatectl targets this).
    let (ipc_tx, ipc_rx) = mpsc::channel::<slate_ipc::IpcRequest>();
    let socket = slate_ipc::default_socket();
    match slate_ipc::spawn_server(&socket, ipc_tx) {
        Ok(()) => driver_log(&mut app, format!("ipc: listening on {}", socket.display())),
        Err(e) => driver_log(&mut app, format!("ipc: disabled — {e}")),
    }

    // Input pump: a thread owns blocking stdin reads; the loop below stays
    // tick-driven via a channel with timeout.
    let (key_tx, key_rx) = mpsc::channel::<Vec<u8>>();
    thread::spawn(move || {
        let stdin = std::io::stdin();
        let mut lock = stdin.lock();
        let mut buf = [0u8; 1024];
        loop {
            match lock.read(&mut buf) {
                Ok(0) => break,       // EOF
                Ok(n) => {
                    if key_tx.send(buf[..n].to_vec()).is_err() {
                        break;        // shell went away
                    }
                }
                Err(_) => break,
            }
        }
    });

    let mut decoder = KeyDecoder::new();
    let tick = app.tick_duration();
    let mut last_tick = Instant::now();
    let mut last_size_check = Instant::now() - Duration::from_secs(60);

    while app.running {
        // 1) IPC requests (slatectl) — execute through the same engine.
        while let Ok(req) = ipc_rx.try_recv() {
            let (ok, text) = app.run_command_text(&req.line);
            let resp = if ok {
                slate_ipc::IpcResponse::ok(text)
            } else {
                slate_ipc::IpcResponse::err(text)
            };
            let _ = req.reply.send(resp);
        }

        // 2) Input (block up to the next tick).
        let wait = tick.saturating_sub(last_tick.elapsed()).max(Duration::from_millis(5));
        match key_rx.recv_timeout(wait) {
            Ok(bytes) => {
                for key in decoder.feed(&bytes) {
                    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        app.handle_key(key)
                    }));
                    match r {
                        Ok(Ok(())) => {}
                        Ok(Err(e)) => driver_log(&mut app, format!("error: {e}")),
                        Err(_) => {
                            let msg = take_panic().unwrap_or_else(|| "pane crash".to_string());
                            driver_log(&mut app, format!("recovered from pane crash: {msg}"));
                        }
                    }
                    if !app.running {
                        break;
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break, // stdin closed
        }

        // 3) Tick (pane refresh, autosave, clocks).
        if last_tick.elapsed() >= tick {
            last_tick = Instant::now();
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| app.on_tick()));
            if r.is_err() {
                let msg = take_panic().unwrap_or_else(|| "tick crash".to_string());
                driver_log(&mut app, format!("recovered from tick crash: {msg}"));
            }
        }

        // 4) Resize polling (bounded; SIGWINCH-free by design).
        if last_size_check.elapsed() >= Duration::from_millis(500) {
            last_size_check = Instant::now();
            if let Ok(true) = term.sync_size() {
                app.dirty = true;
            }
        }

        // 5) Draw only when dirty.
        if app.dirty {
            // The cursor computed by the previous frame is applied now
            // (documented one-frame lag; keeps draw_frame &mut-friendly).
            term.set_cursor_position(app.cursor_out);
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                term.draw(|buf, area| app.draw_frame(buf, area))
            }));
            match r {
                Ok(Ok(())) => app.dirty = false,
                Ok(Err(e)) => {
                    // The screen is gone (EPIPE etc.): no session without it.
                    let _ = take_panic();
                    eprintln!("\r\n{APP_NAME}: terminal write failed: {e}");
                    break;
                }
                Err(_) => {
                    let msg = take_panic().unwrap_or_else(|| "render crash".to_string());
                    driver_log(&mut app, format!("recovered from render crash: {msg}"));
                    // Mark clean to avoid a crash loop on the same frame.
                    app.dirty = false;
                }
            }
        }
    }

    // Teardown: alt-screen/cursor first, then stty restore (RawMode::drop),
    // then the IPC socket. Order matters; keep it boring.
    {
        let mut out = std::io::stdout();
        let _ = out.write_all(b"\x1b[0m\x1b[?25h\x1b[?1049l");
        let _ = out.flush();
    }
    drop(raw);
    let _ = std::fs::remove_file(&socket);
    Ok(EXIT_OK)
}
