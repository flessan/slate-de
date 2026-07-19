//! `slate-ipc` — the Slate control socket.
//!
//! A running `slate` session listens on `$XDG_RUNTIME_DIR/slate.sock`;
//! `slatectl` connects to script the desktop from any shell:
//!
//! ```text
//! $ slatectl workspace switch coding
//! $ slatectl notes add "remember the milk"
//! ```
//!
//! # Protocol
//!
//! Length-framed (u32 little-endian length prefix) with a one-byte tag:
//!
//! * `0x01` request: UTF-8 command line
//! * `0x02` ok response: UTF-8 output text (may be empty)
//! * `0x03` error response: UTF-8 error message
//!
//! Deliberately simple: framing is 20 lines, there is no serialization
//! library in the workspace, and the entire surface is plain command lines —
//! the same DSL the user types into the prompt.

#![forbid(unsafe_code)]

use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;

use slate_core::error::{Error, Result};

const TAG_REQUEST: u8 = 0x01;
const TAG_OK: u8 = 0x02;
const TAG_ERR: u8 = 0x03;
const MAX_FRAME: u32 = 256 * 1024;

/// Default socket location (`$XDG_RUNTIME_DIR/slate.sock`).
pub fn default_socket() -> PathBuf {
    slate_config::paths::socket_file()
}

/// One incoming request: the raw command line + reply channel.
pub struct IpcRequest {
    pub line: String,
    pub reply: mpsc::Sender<IpcResponse>,
}

/// The shell's answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IpcResponse {
    pub ok: bool,
    pub output: String,
}

impl IpcResponse {
    pub fn ok(output: String) -> Self {
        IpcResponse { ok: true, output }
    }

    pub fn err(msg: String) -> Self {
        IpcResponse { ok: false, output: msg }
    }
}

fn write_frame(w: &mut impl Write, tag: u8, payload: &str) -> std::io::Result<()> {
    let bytes = payload.as_bytes();
    let len = bytes.len() as u32 + 1;
    if len > MAX_FRAME {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "frame too large"));
    }
    w.write_all(&len.to_le_bytes())?;
    w.write_all(&[tag])?;
    w.write_all(bytes)?;
    w.flush()
}

fn read_frame(r: &mut impl Read) -> std::io::Result<Option<(u8, String)>> {
    let mut len_buf = [0u8; 4];
    match r.read_exact(&mut len_buf) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }
    let len = u32::from_le_bytes(len_buf);
    if len == 0 || len > MAX_FRAME {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "bad frame length"));
    }
    let mut buf = vec![0u8; len as usize];
    r.read_exact(&mut buf)?;
    let tag = buf[0];
    Ok(Some((tag, String::from_utf8_lossy(&buf[1..]).into_owned())))
}

/// Send a command; returns the response (`ok` flag + payload).
pub fn send(path: &std::path::Path, line: &str) -> Result<IpcResponse> {
    let mut stream = UnixStream::connect(path)
        .map_err(|e| Error::unavailable(format!("no slate session on {}: {e}", path.display())))?;
    write_frame(&mut stream, TAG_REQUEST, line)?;
    match read_frame(&mut stream)? {
        Some((TAG_OK, text)) => Ok(IpcResponse::ok(text)),
        Some((TAG_ERR, text)) => Ok(IpcResponse::err(text)),
        Some((other, _)) => Err(Error::other(format!("unexpected frame tag {other:#x}"))),
        None => Err(Error::other("connection closed without a response")),
    }
}

/// Start the listening thread; requests arrive on `tx` and must be answered
/// through each request's reply channel.
pub fn spawn_server(path: &PathBuf, tx: mpsc::Sender<IpcRequest>) -> Result<()> {
    // Recover from stale sockets: only remove when nothing is listening.
    if path.exists() {
        match UnixStream::connect(path) {
            Ok(_) => return Err(Error::other(format!("another slate owns {}", path.display()))),
            Err(_) => {
                let _ = std::fs::remove_file(path);
            }
        }
    }
    let listener = UnixListener::bind(path)?;
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut s) = stream else { continue };
            let tx = tx.clone();
            thread::spawn(move || {
                if let Ok(Some((TAG_REQUEST, line))) = read_frame(&mut s) {
                    let (reply_tx, reply_rx) = mpsc::channel();
                    if tx.send(IpcRequest { line, reply: reply_tx }).is_err() {
                        return; // shell went away
                    }
                    if let Ok(resp) = reply_rx.recv() {
                        let tag = if resp.ok { TAG_OK } else { TAG_ERR };
                        let _ = write_frame(&mut s, tag, &resp.output);
                    }
                }
            });
        }
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_over_socket() {
        let dir = std::env::temp_dir().join(format!("slate-ipc-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let sock = dir.join("test.sock");
        let _ = std::fs::remove_file(&sock);

        let (tx, rx) = mpsc::channel::<IpcRequest>();
        spawn_server(&sock, tx).unwrap();

        let client_sock = sock.clone();
        let handle = thread::spawn(move || send(&client_sock, "echo hello").unwrap());

        let req = rx.recv().unwrap();
        assert_eq!(req.line, "echo hello");
        req.reply.send(IpcResponse::ok("hello".into())).unwrap();
        let resp = handle.join().unwrap();
        assert!(resp.ok);
        assert_eq!(resp.output, "hello");

        // Error path.
        let client_sock = sock.clone();
        let handle = thread::spawn(move || send(&client_sock, "explode").unwrap());
        let req = rx.recv().unwrap();
        req.reply.send(IpcResponse::err("boom".into())).unwrap();
        let resp = handle.join().unwrap();
        assert!(!resp.ok);
        assert_eq!(resp.output, "boom");

        drop(rx);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn stale_socket_is_replaced() {
        let dir = std::env::temp_dir().join(format!("slate-ipc-stale-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let sock = dir.join("stale.sock");
        std::fs::write(&sock, b"junk").unwrap(); // dead file, nobody listening
        let (tx, _rx) = mpsc::channel::<IpcRequest>();
        assert!(spawn_server(&sock, tx).is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn connecting_to_nothing_reports_unavailable() {
        let resp = send(&std::env::temp_dir().join("slate-no-such-sock"), "x");
        assert!(resp.is_err());
    }
}
