//! `slatectl` — drive a running slate desktop from any shell.
//!
//! Everything slate can do is a command line; `slatectl` sends those same
//! command lines to the session's control socket (`slate-ipc`). This is the
//! scriptable surface for keybinding daemons, status bars, cron jobs, and
//! CI:
//!
//! ```text
//! $ slatectl workspace switch coding
//! $ slatectl notes add "deploy the thing"
//! $ slatectl theme set slate-light
//! $ slatectl ping
//! ```
//!
//! Exit codes: 0 ok · 1 session/command error · 2 usage.

#![forbid(unsafe_code)]

use slate_core::args::{take_flag, take_option};
use slate_core::VERSION;

const USAGE: &str = "\
slatectl — control a running slate session

USAGE
  slatectl <command line…>      send a command line to the session
  slatectl ping                 check that a session is alive
  slatectl -s PATH <command…>   use a non-default socket
  slatectl --help               this text

EXAMPLES
  slatectl workspace switch coding
  slatectl notes add \"deploy the thing\"
  slatectl run search docs
  slatectl ping && echo up

The command language is identical to the in-app prompt; see `slate keys`
and `slate help` inside a session, or docs/COMMANDS.md.

Exit codes: 0 ok · 1 session/command error · 2 usage.
";

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::process::ExitCode::from(dispatch(args))
}

fn dispatch(mut args: Vec<String>) -> u8 {
    if take_flag(&mut args, &["-h", "--help"]) {
        print!("{USAGE}");
        return 0;
    }
    if take_flag(&mut args, &["-V", "--version"]) {
        println!("slatectl {VERSION}");
        return 0;
    }
    let socket = take_option(&mut args, &["-s", "--socket"])
        .map(std::path::PathBuf::from)
        .unwrap_or_else(slate_ipc::default_socket);
    let ping = args.first().map(String::as_str) == Some("ping") && args.len() == 1;
    let line = if ping { "about".to_string() } else { args.join(" ") };
    if line.trim().is_empty() {
        eprintln!("slatectl: no command given\n");
        print!("{USAGE}");
        return 2;
    }
    match slate_ipc::send(&socket, &line) {
        Ok(resp) if resp.ok => {
            if ping {
                println!("pong — slate session alive at {}", socket.display());
            } else if !resp.output.trim().is_empty() {
                println!("{}", resp.output);
            }
            0
        }
        Ok(resp) => {
            eprintln!("slatectl: {}", resp.output);
            1
        }
        Err(e) => {
            eprintln!("slatectl: {e}");
            eprintln!("hint: start a desktop session with `slate` (or use `slate run` headlessly)");
            1
        }
    }
}
