//! System probes: `/proc`, `/sysfs`, and small helper commands.
//!
//! Everything degrades gracefully — a missing battery, absent interface, or
//! missing tool yields `None`, never a crash. Probes hold state across
//! samples (CPU deltas, network rates).

use std::fs;
use std::path::Path;

use slate_core::proc;

/// Battery reading.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Battery {
    pub name: String,
    pub percent: u8,
    pub status: String, // "Charging", "Discharging", "Full", …
}

/// A network interface reading.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Iface {
    pub name: String,
    pub up: bool,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub kind: IfaceKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IfaceKind {
    Loopback,
    Vpn,
    Regular,
}

/// Stateful system probe (rate-based metrics need previous samples).
#[derive(Debug, Default)]
pub struct Probe {
    cpu_prev: Option<(u64, u64)>, // (idle_all, total) jiffies
    net_prev: Option<(u64, u64, u64)>, // (rx, tx, unix secs)
}

fn read(path: &Path) -> Option<String> {
    fs::read_to_string(path).ok()
}

fn read_trimmed(path: &Path) -> Option<String> {
    read(path).map(|s| s.trim().to_string())
}

impl Probe {
    pub fn new() -> Self {
        Probe::default()
    }

    // ---- CPU ------------------------------------------------------------

    /// CPU usage since last call (0.0–1.0).
    pub fn cpu_fraction(&mut self) -> f64 {
        let Some((idle, total)) = cpu_jiffies() else { return 0.0 };
        let frac = match self.cpu_prev {
            Some((p_idle, p_total)) if total > p_total => {
                let d_total = total - p_total;
                let d_idle = idle.saturating_sub(p_idle);
                1.0 - (d_idle as f64 / d_total as f64)
            }
            _ => 0.0,
        };
        self.cpu_prev = Some((idle, total));
        frac.clamp(0.0, 1.0)
    }

    // ---- Memory ---------------------------------------------------------

    /// `(used, total)` bytes of RAM.
    pub fn memory(&self) -> (u64, u64) {
        let Some(meminfo) = read(Path::new("/proc/meminfo")) else { return (0, 0) };
        let mut total = 0u64;
        let mut avail = 0u64;
        for line in meminfo.lines() {
            if let Some(v) = line.strip_prefix("MemTotal:") {
                total = kb_field(v);
            } else if let Some(v) = line.strip_prefix("MemAvailable:") {
                avail = kb_field(v);
            }
        }
        (total.saturating_sub(avail), total)
    }

    /// `(used, total)` bytes of swap.
    pub fn swap(&self) -> (u64, u64) {
        let Some(meminfo) = read(Path::new("/proc/meminfo")) else { return (0, 0) };
        let mut total = 0u64;
        let mut free = 0u64;
        for line in meminfo.lines() {
            if let Some(v) = line.strip_prefix("SwapTotal:") {
                total = kb_field(v);
            } else if let Some(v) = line.strip_prefix("SwapFree:") {
                free = kb_field(v);
            }
        }
        (total.saturating_sub(free), total)
    }

    // ---- Load / uptime ----------------------------------------------------

    pub fn loadavg(&self) -> (f64, f64, f64) {
        let Some(s) = read(Path::new("/proc/loadavg")) else { return (0.0, 0.0, 0.0) };
        let mut it = s.split_whitespace();
        let p = |s: Option<&str>| s.and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
        (p(it.next()), p(it.next()), p(it.next()))
    }

    pub fn uptime_secs(&self) -> u64 {
        read(Path::new("/proc/uptime"))
            .and_then(|s| s.split_whitespace().next()?.parse::<f64>().ok())
            .unwrap_or(0.0) as u64
    }

    // ---- Disk -------------------------------------------------------------

    /// `(used, total)` bytes of the filesystem mounted at `/` (via `df`).
    pub fn disk_root(&self) -> (u64, u64) {
        let Some(out) = proc::run_trimmed("df", &["-P", "-B1", "/"]) else {
            return (0, 0);
        };
        let Some(line) = out.lines().nth(1) else { return (0, 0) };
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.len() < 4 {
            return (0, 0);
        }
        let total = f[1].parse::<u64>().unwrap_or(0);
        let used = f[2].parse::<u64>().unwrap_or(0);
        (used, total)
    }

    // ---- Battery ----------------------------------------------------------

    pub fn battery(&self) -> Option<Battery> {
        let dir = Path::new("/sys/class/power_supply");
        let rd = fs::read_dir(dir).ok()?;
        for entry in rd.flatten() {
            let path = entry.path();
            if read_trimmed(&path.join("type")).as_deref() != Some("Battery") {
                continue;
            }
            let percent = read_trimmed(&path.join("capacity"))
                .and_then(|s| s.parse::<u8>().ok())
                .unwrap_or(0);
            let status =
                read_trimmed(&path.join("status")).unwrap_or_else(|| "Unknown".to_string());
            let name = entry.file_name().to_string_lossy().into_owned();
            return Some(Battery { name, percent, status });
        }
        None
    }

    // ---- Network ----------------------------------------------------------

    pub fn interfaces(&self) -> Vec<Iface> {
        let base = Path::new("/sys/class/net");
        let mut out = Vec::new();
        let Ok(rd) = fs::read_dir(base) else { return out };
        for entry in rd.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let p = entry.path();
            let up = read_trimmed(&p.join("operstate")).as_deref() == Some("up");
            let read_u64 = |f: &str| {
                read_trimmed(&p.join(f)).and_then(|s| s.parse::<u64>().ok()).unwrap_or(0)
            };
            let kind = if name == "lo" {
                IfaceKind::Loopback
            } else if name.starts_with("tun")
                || name.starts_with("wg")
                || name.starts_with("ppp")
                || name.starts_with("tailscale")
            {
                IfaceKind::Vpn
            } else {
                IfaceKind::Regular
            };
            out.push(Iface {
                name,
                up,
                rx_bytes: read_u64("statistics/rx_bytes"),
                tx_bytes: read_u64("statistics/tx_bytes"),
                kind,
            });
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
        out
    }

    /// Aggregate `(rx, tx)` bytes/sec across non-loopback interfaces.
    pub fn net_rates(&mut self) -> (u64, u64) {
        let ifs = self.interfaces();
        let rx: u64 = ifs.iter().filter(|i| i.kind != IfaceKind::Loopback).map(|i| i.rx_bytes).sum();
        let tx: u64 = ifs.iter().filter(|i| i.kind != IfaceKind::Loopback).map(|i| i.tx_bytes).sum();
        let now = slate_core::civil::DateTime::now_utc().secs as u64;
        let rates = match self.net_prev {
            Some((p_rx, p_tx, p_t)) if now > p_t => {
                ((rx.saturating_sub(p_rx)) / (now - p_t), (tx.saturating_sub(p_tx)) / (now - p_t))
            }
            _ => (0, 0),
        };
        self.net_prev = Some((rx, tx, now));
        rates
    }

    /// Name of an active VPN interface, if any.
    pub fn active_vpn(&self) -> Option<String> {
        self.interfaces()
            .into_iter()
            .find(|i| i.kind == IfaceKind::Vpn && i.up)
            .map(|i| i.name)
    }
}

fn kb_field(s: &str) -> u64 {
    s.split_whitespace().next().and_then(|v| v.parse::<u64>().ok()).unwrap_or(0) * 1024
}

fn cpu_jiffies() -> Option<(u64, u64)> {
    let stat = read(Path::new("/proc/stat"))?;
    let line = stat.lines().next()?;
    let body = line.strip_prefix("cpu ")?;
    let mut total = 0u64;
    let mut idle = 0u64;
    for (i, v) in body.split_whitespace().enumerate() {
        let j: u64 = v.parse().ok()?;
        total += j;
        if i == 3 || i == 4 {
            idle += j; // idle + iowait
        }
    }
    Some((idle, total))
}

/// Format bytes as `1.4 GiB`.
pub fn human_bytes(n: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut val = n as f64;
    let mut unit = 0;
    while val >= 1024.0 && unit < UNITS.len() - 1 {
        val /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{n} B")
    } else {
        format!("{val:.1} {}", UNITS[unit])
    }
}

/// Format bytes/sec as `1.4 MiB/s`.
pub fn human_rate(n: u64) -> String {
    format!("{}/s", human_bytes(n))
}

/// Format seconds as `3d 4h 12m`.
pub fn human_uptime(secs: u64) -> String {
    let d = secs / 86_400;
    let h = secs % 86_400 / 3600;
    let m = secs % 3600 / 60;
    if d > 0 {
        format!("{d}d {h}h {m}m")
    } else if h > 0 {
        format!("{h}h {m}m")
    } else {
        format!("{m}m")
    }
}

// ---------------------------------------------------------------------------
// command-backed probes (widgets + system commands share these)
// ---------------------------------------------------------------------------

/// Output volume via `wpctl` (PipeWire): `(percent, muted)`.
pub fn volume_state() -> Option<(u8, bool)> {
    let out = proc::run_trimmed("wpctl", &["get-volume", "@DEFAULT_AUDIO_SINK@"])?;
    let muted = out.contains("MUTED");
    let ratio: f64 = out
        .split(':')
        .nth(1)?
        .split_whitespace()
        .next()?
        .parse()
        .ok()?;
    Some((((ratio * 100.0).round() as u8).min(100), muted))
}

/// Set output volume in percent via `wpctl`.
pub fn set_volume(pct: u8) -> Option<String> {
    let pct = pct.min(150);
    proc::run("wpctl", &["set-volume", "@DEFAULT_AUDIO_SINK@", &format!("{pct}%")])
        .ok()
        .filter(|o| o.status.success())
        .and_then(|_| volume_state())
        .map(|(p, m)| format!("{p}%{}", if m { " (muted)" } else { "" }))
}

/// Now-playing via `playerctl`.
pub fn now_playing() -> Option<String> {
    let out = proc::run_trimmed("playerctl", &["metadata", "--format", "{{ artist }} - {{ title }}"])?;
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

 /// Git state of a directory: `(branch, dirty files)`.
pub fn git_state(cwd: &std::path::Path) -> (Option<String>, usize) {
    let dir = cwd.to_string_lossy();
    let branch = proc::run_trimmed(
    "git",
    &["-C", &dir, "rev-parse", "--abbrev-ref", "HEAD"]
);
    let dirty = proc::run_trimmed("git", &["-C", &dir, "status", "--porcelain"])
        .map(|s| s.lines().filter(|l| !l.trim().is_empty()).count())
        .unwrap_or(0);
    (branch, dirty)
}

/// One-line weather from wttr.in (opt-in; needs `curl` and network).
pub fn fetch_weather(location: &str) -> Option<String> {
    let url = format!("https://wttr.in/{}?format=%c+%t", slate_core::url::percent_encode(location));
    let out = proc::run_trimmed("curl", &["-s", "--max-time", "3", &url])?;
    if out.is_empty() || out.contains('<') {
        // wttr.in error pages are HTML — don't render markup.
        None
    } else {
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn human_formats() {
        assert_eq!(human_bytes(512), "512 B");
        assert_eq!(human_bytes(2048), "2.0 KiB");
        assert_eq!(human_bytes(5 * 1024 * 1024), "5.0 MiB");
        assert_eq!(human_rate(1024), "1.0 KiB/s");
        assert_eq!(human_uptime(65), "1m");
        assert_eq!(human_uptime(3_720), "1h 2m");
        assert_eq!(human_uptime(180_000), "2d 2h 0m");
    }

    #[test]
    fn probes_do_not_panic() {
        // CI machines are real Linux: /proc must answer.
        let mut p = Probe::new();
        let (_used, total) = p.memory();
        assert!(total > 0);
        assert!(p.uptime_secs() > 0);
        let _ = p.cpu_fraction();
        let _ = p.net_rates();
        let _ = p.disk_root();
        let _ = p.battery();
        let ifs = p.interfaces();
        assert!(ifs.iter().any(|i| i.name == "lo"));
    }
}
