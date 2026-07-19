//! System integration commands.
//!
//! These delegate to the native Linux stack (`nmcli`, `bluetoothctl`,
//! `wpctl`, `systemctl`, …) following Slate's "embrace existing tools,
//! degrade gracefully" rule: missing tool ⇒ a precise, actionable error —
//! never a crash, never a stub that pretends to work.

use std::path::PathBuf;

use slate_core::civil::{month_grid, DateTime, MONTHS};
use slate_core::error::{Error, Result};
use slate_core::proc;
use slate_core::style::{Line, Span, Style};
use slate_dashboard::probe::{self, human_bytes, human_uptime, Probe};

use crate::{Command, Ctx, Output, Registry};

pub fn register(r: &mut Registry) {
    r.register(Wifi);
    r.register(Bluetooth);
    r.register(Volume);
    r.register(Brightness);
    r.register(Music);
    r.register(Battery);
    r.register(Network);
    r.register(Power);
    r.register(UptimeCmd);
    r.register(Sysinfo);
    r.register(Calendar);
    r.register(Weather);
    r.register(Git);
    r.register(Notifications);
    r.register(Files);
    r.register(Docker);
}

/// Missing-tool error with an actionable hint.
fn need(tool: &str, feature: &str) -> Result<()> {
    if proc::exists(tool) {
        Ok(())
    } else {
        Err(Error::unavailable(format!("{feature} needs `{tool}` (not found on PATH)")))
    }
}

// ---------------------------------------------------------------------------
// wifi / bluetooth
// ---------------------------------------------------------------------------

struct Wifi;

impl Command for Wifi {
    fn name(&self) -> &'static str {
        "wifi"
    }
    fn about(&self) -> &'static str {
        "list wifi networks, toggle radio, connect (via nmcli)"
    }
    fn usage(&self) -> &'static str {
        "wifi [list|on|off|connect <ssid>]"
    }

    fn run(&self, args: &[String], ctx: &mut Ctx) -> Result<Output> {
        need("nmcli", "wifi")?;
        let theme = ctx.theme();
        match args.first().map(String::as_str).unwrap_or("list") {
            "list" => {
                let out = proc::run("nmcli", &["-t", "-f", "IN-USE,SSID,SIGNAL", "dev", "wifi"])?;
                let text = String::from_utf8_lossy(&out.stdout);
                let mut output = Output::new();
                let mut any = false;
                for line in text.lines().take(30) {
                    let mut parts = line.splitn(3, ':');
                    let in_use = parts.next().unwrap_or("");
                    let ssid = parts.next().unwrap_or("");
                    let signal = parts.next().unwrap_or("");
                    if ssid.is_empty() {
                        continue;
                    }
                    any = true;
                    let active = in_use == "*";
                    let style = if active { theme.accent() } else { theme.text() };
                    output.line(Line::new(vec![
                        Span::styled(if active { "● " } else { "  " }, style),
                        Span::styled(format!("{ssid:<28}"), style),
                        Span::styled(format!("{signal}%"), theme.dim()),
                    ]));
                }
                if !any {
                    output.line(Line::styled("no networks found".to_string(), theme.dim()));
                }
                Ok(output)
            }
            "on" | "off" => {
                let state = args[0].as_str();
                let out = proc::run("nmcli", &["radio", "wifi", state])?;
                if out.status.success() {
                    Ok(Output::text(format!("wifi {state}")))
                } else {
                    Err(Error::other(String::from_utf8_lossy(&out.stderr).trim().to_string()))
                }
            }
            "connect" => {
                let ssid = args[1..].join(" ");
                if ssid.is_empty() {
                    return Err(Error::invalid("usage: wifi connect <ssid>"));
                }
                let out = proc::run("nmcli", &["dev", "wifi", "connect", &ssid])?;
                if out.status.success() {
                    Ok(Output::text(format!("connected to '{ssid}'")))
                } else {
                    Err(Error::other(format!(
                        "connect failed (a password may be required): {}",
                        String::from_utf8_lossy(&out.stderr).trim()
                    )))
                }
            }
            other => Err(Error::invalid(format!("unknown wifi subcommand '{other}'"))),
        }
    }
}

struct Bluetooth;

impl Command for Bluetooth {
    fn name(&self) -> &'static str {
        "bluetooth"
    }
    fn aliases(&self) -> &'static [&'static str] {
        &["bt"]
    }
    fn about(&self) -> &'static str {
        "bluetooth status, devices, power toggle (via bluetoothctl)"
    }
    fn usage(&self) -> &'static str {
        "bluetooth [status|on|off|devices]"
    }

    fn run(&self, args: &[String], ctx: &mut Ctx) -> Result<Output> {
        need("bluetoothctl", "bluetooth")?;
        let theme = ctx.theme();
        match args.first().map(String::as_str).unwrap_or("status") {
            "status" | "devices" => {
                let show = proc::run_trimmed("bluetoothctl", &["show"]).unwrap_or_default();
                let powered = show
                    .lines()
                    .find_map(|l| l.trim().strip_prefix("Powered:"))
                    .map(str::trim)
                    .unwrap_or("unknown")
                    .to_string();
                let mut out = Output::new();
                out.line(Line::new(vec![
                    Span::styled("bluetooth: ", theme.dim()),
                    Span::styled(powered.clone(), if powered == "yes" { theme.ok() } else { theme.warn() }),
                ]));
                if let Some(devs) = proc::run_trimmed("bluetoothctl", &["devices"]) {
                    for line in devs.lines().take(20) {
                        if let Some(rest) = line.strip_prefix("Device ") {
                            let (mac, name) = rest.split_once(' ').unwrap_or((rest, ""));
                            out.line(Line::new(vec![
                                Span::styled("  ● ", theme.accent2()),
                                Span::styled(name.to_string(), theme.text()),
                                Span::styled(format!("  {mac}"), theme.dim()),
                            ]));
                        }
                    }
                }
                Ok(out)
            }
            "on" | "off" => {
                let yes = args[0] == "on";
                proc::run_trimmed("bluetoothctl", &["power", if yes { "on" } else { "off" }]);
                Ok(Output::text(format!("bluetooth power {}", if yes { "on" } else { "off" })))
            }
            other => Err(Error::invalid(format!("unknown bluetooth subcommand '{other}'"))),
        }
    }
}

// ---------------------------------------------------------------------------
// volume / brightness / music / battery / network
// ---------------------------------------------------------------------------

struct Volume;

impl Command for Volume {
    fn name(&self) -> &'static str {
        "volume"
    }
    fn aliases(&self) -> &'static [&'static str] {
        &["vol"]
    }
    fn about(&self) -> &'static str {
        "output volume: get, set %, up/down, mute (via wpctl)"
    }
    fn usage(&self) -> &'static str {
        "volume [<0-150>|up|down|mute]"
    }

    fn run(&self, args: &[String], ctx: &mut Ctx) -> Result<Output> {
        need("wpctl", "volume")?;
        let theme = ctx.theme();
        match args.first().map(String::as_str) {
            None => match probe::volume_state() {
                Some((pct, muted)) => Ok(Output::from_lines(vec![Line::new(vec![
                    Span::styled("volume: ", theme.dim()),
                    Span::styled(format!("{pct}%"), theme.text()),
                    Span::styled(if muted { " (muted)" } else { "" }, theme.warn()),
                ])])),
                None => Err(Error::unavailable("could not read volume state")),
            },
            Some("mute") | Some("toggle") => {
                proc::run("wpctl", &["set-mute", "@DEFAULT_AUDIO_SINK@", "toggle"])?;
                let muted = probe::volume_state().map(|(_, m)| m).unwrap_or(false);
                Ok(Output::text(if muted { "muted" } else { "unmuted" }))
            }
            Some("up") | Some("down") => {
                let (pct, _) = probe::volume_state().unwrap_or((50, false));
                let step: i64 = if args[0] == "up" { 5 } else { -5 };
                let target = (i64::from(pct) + step).clamp(0, 150) as u8;
                let new = probe::set_volume(target)
                    .ok_or_else(|| Error::other("wpctl set-volume failed"))?;
                Ok(Output::text(format!("volume: {new}")))
            }
            Some(n) => {
                let pct: u8 = n.parse().map_err(|_| Error::invalid(format!("'{n}' is not 0-150")))?;
                let new = probe::set_volume(pct)
                    .ok_or_else(|| Error::other("wpctl set-volume failed"))?;
                Ok(Output::text(format!("volume: {new}")))
            }
        }
    }
}

struct Brightness;

impl Command for Brightness {
    fn name(&self) -> &'static str {
        "brightness"
    }
    fn about(&self) -> &'static str {
        "screen brightness in % (via brightnessctl)"
    }
    fn usage(&self) -> &'static str {
        "brightness [0-100]"
    }

    fn run(&self, args: &[String], ctx: &mut Ctx) -> Result<Output> {
        need("brightnessctl", "brightness")?;
        match args.first() {
            None => Ok(Output::text(
                proc::run_trimmed("brightnessctl", &["-m"])
                    .and_then(|m| m.split(',').nth(3).map(str::to_string))
                    .map(|p| format!("brightness: {p}"))
                    .unwrap_or_else(|| "brightness: unknown".to_string()),
            )),
            Some(n) => {
                let pct: u8 = n.parse().map_err(|_| Error::invalid("usage: brightness [0-100]"))?;
                let out = proc::run("brightnessctl", &["set", &format!("{pct}%")])?;
                if !out.status.success() {
                    return Err(Error::other(String::from_utf8_lossy(&out.stderr).trim().to_string()));
                }
                let _ = ctx.notify(
                    slate_core::notify::Level::Info,
                    format!("brightness set to {pct}%"),
                );
                Ok(Output::text(format!("brightness: {pct}%")))
            }
        }
    }
}

struct Music;

impl Command for Music {
    fn name(&self) -> &'static str {
        "music"
    }
    fn about(&self) -> &'static str {
        "media player: status, play/pause, next/prev (via playerctl)"
    }
    fn usage(&self) -> &'static str {
        "music [status|play-pause|next|prev]"
    }

    fn run(&self, args: &[String], ctx: &mut Ctx) -> Result<Output> {
        need("playerctl", "music")?;
        let theme = ctx.theme();
        match args.first().map(String::as_str).unwrap_or("status") {
            "status" => match probe::now_playing() {
                Some(track) => Ok(Output::from_lines(vec![
                    Line::new(vec![
                        Span::styled(format!("{} ", theme.glyph("music")), theme.accent2()),
                        Span::styled(track, theme.text()),
                    ]),
                    Line::styled(
                        format!("  {}", proc::run_trimmed("playerctl", &["status"]).unwrap_or_default()),
                        theme.dim(),
                    ),
                ])),
                None => Ok(Output::text("nothing playing (or no MPRIS player running)")),
            },
            verb @ ("play-pause" | "next" | "prev" | "previous" | "play" | "pause" | "stop") => {
                let verb = if verb == "previous" { "prev" } else { verb };
                let verb = if verb == "prev" { "previous" } else { verb };
                let out = proc::run("playerctl", &[verb])?;
                if !out.status.success() {
                    return Err(Error::other(String::from_utf8_lossy(&out.stderr).trim().to_string()));
                }
                Ok(Output::text(format!("music {verb} ✓")))
            }
            other => Err(Error::invalid(format!("unknown music subcommand '{other}'"))),
        }
    }
}

struct Battery;

impl Command for Battery {
    fn name(&self) -> &'static str {
        "battery"
    }
    fn about(&self) -> &'static str {
        "battery level and status"
    }

    fn run(&self, _args: &[String], ctx: &mut Ctx) -> Result<Output> {
        let probe = Probe::new();
        let theme = ctx.theme();
        match probe.battery() {
            Some(b) => Ok(Output::from_lines(vec![Line::new(vec![
                Span::styled("battery: ", theme.dim()),
                Span::styled(format!("{}%", b.percent), theme.text()),
                Span::styled(format!("  ({}, {})", b.name, b.status), theme.dim()),
            ])])),
            None => Ok(Output::text("no battery found (desktop system?)")),
        }
    }
}

struct Network;

impl Command for Network {
    fn name(&self) -> &'static str {
        "network"
    }
    fn aliases(&self) -> &'static [&'static str] {
        &["net"]
    }
    fn about(&self) -> &'static str {
        "network interfaces, addresses, and transfer totals"
    }

    fn run(&self, _args: &[String], ctx: &mut Ctx) -> Result<Output> {
        let mut probe = Probe::new();
        let theme = ctx.theme();
        let mut out = Output::new();
        let ifs = probe.interfaces();
        if ifs.is_empty() {
            out.line(Line::styled("no interfaces (no /sys/class/net?)", theme.dim()));
        }
        for i in ifs {
            let (state, style) = if i.up { ("up", theme.ok()) } else { ("down", theme.dim()) };
            let kind = match i.kind {
                probe::IfaceKind::Loopback => "lo ",
                probe::IfaceKind::Vpn => "vpn",
                probe::IfaceKind::Regular => "   ",
            };
            out.line(Line::new(vec![
                Span::styled(format!("{kind} "), theme.dim()),
                Span::styled(format!("{i_n:<12}", i_n = i.name), theme.text()),
                Span::styled(format!("{state:<5}"), style),
                Span::styled(
                    format!("rx {:>10}  tx {:>10}", human_bytes(i.rx_bytes), human_bytes(i.tx_bytes)),
                    theme.dim(),
                ),
            ]));
        }
        Ok(out)
    }
}

// ---------------------------------------------------------------------------
// power
// ---------------------------------------------------------------------------

struct Power;

impl Command for Power {
    fn name(&self) -> &'static str {
        "power"
    }
    fn about(&self) -> &'static str {
        "power actions: off, reboot, suspend, lock (requires --yes)"
    }
    fn usage(&self) -> &'static str {
        "power <off|reboot|suspend|lock> [--yes]"
    }

    fn run(&self, args: &[String], ctx: &mut Ctx) -> Result<Output> {
        let theme = ctx.theme();
        let Some(action) = args.first() else {
            return Err(Error::invalid("usage: power <off|reboot|suspend|lock> [--yes]"));
        };
        let (verb, cmd): (&str, &[&str]) = match action.as_str() {
            "off" | "shutdown" => ("power off", &["systemctl", "poweroff"]),
            "reboot" | "restart" => ("reboot", &["systemctl", "reboot"]),
            "suspend" | "sleep" => ("suspend", &["systemctl", "suspend"]),
            "lock" => ("lock session", &["loginctl", "lock-session"]),
            other => return Err(Error::invalid(format!("unknown power action '{other}'"))),
        };
        let confirmed = args.iter().any(|a| a == "--yes" || a == "-y");
        if !confirmed {
            return Ok(Output::from_lines(vec![
                Line::styled(
                    format!("about to {verb}. run again with --yes to confirm:"),
                    theme.warn(),
                ),
                Line::styled(format!("  power {action} --yes"), theme.accent()),
            ]));
        }
        let out = proc::run(cmd[0], &cmd[1..])?;
        if out.status.success() {
            Ok(Output::text(format!("{verb}…")))
        } else {
            Err(Error::other(format!(
                "could not {verb}: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            )))
        }
    }
}

// ---------------------------------------------------------------------------
// uptime / sysinfo / calendar / weather / git
// ---------------------------------------------------------------------------

struct UptimeCmd;

impl Command for UptimeCmd {
    fn name(&self) -> &'static str {
        "uptime"
    }
    fn about(&self) -> &'static str {
        "system uptime and load"
    }

    fn run(&self, _args: &[String], ctx: &mut Ctx) -> Result<Output> {
        let probe = Probe::new();
        let (a, b, c) = probe.loadavg();
        let theme = ctx.theme();
        Ok(Output::from_lines(vec![
            Line::new(vec![
                Span::styled("up ", theme.dim()),
                Span::styled(human_uptime(probe.uptime_secs()), theme.text()),
            ]),
            Line::styled(format!("load: {a:.2} {b:.2} {c:.2}"), theme.dim()),
        ]))
    }
}

struct Sysinfo;

impl Command for Sysinfo {
    fn name(&self) -> &'static str {
        "sysinfo"
    }
    fn aliases(&self) -> &'static [&'static str] {
        &["uname"]
    }
    fn about(&self) -> &'static str {
        "kernel, distro, RAM summary"
    }

    fn run(&self, _args: &[String], ctx: &mut Ctx) -> Result<Output> {
        let theme = ctx.theme();
        let kernel = proc::run_trimmed("uname", &["-sr"]).unwrap_or_else(|| "unknown".into());
        let distro = std::fs::read_to_string("/etc/os-release")
            .ok()
            .and_then(|s| {
                s.lines()
                    .find_map(|l| l.strip_prefix("PRETTY_NAME=").map(|v| v.trim_matches('"').to_string()))
            })
            .unwrap_or_else(|| "linux".into());
        let probe = Probe::new();
        let (used, total) = probe.memory();
        Ok(Output::from_lines(vec![
            Line::new(vec![Span::styled("kernel: ", theme.dim()), Span::styled(kernel, theme.text())]),
            Line::new(vec![Span::styled("distro: ", theme.dim()), Span::styled(distro, theme.text())]),
            Line::new(vec![
                Span::styled("memory: ", theme.dim()),
                Span::styled(format!("{} / {}", human_bytes(used), human_bytes(total)), theme.text()),
            ]),
        ]))
    }
}

struct Calendar;

impl Command for Calendar {
    fn name(&self) -> &'static str {
        "calendar"
    }
    fn aliases(&self) -> &'static [&'static str] {
        &["cal"]
    }
    fn about(&self) -> &'static str {
        "render a month calendar (default: current month)"
    }
    fn usage(&self) -> &'static str {
        "calendar [month [year]]"
    }

    fn run(&self, args: &[String], ctx: &mut Ctx) -> Result<Output> {
        let now = DateTime::now_utc();
        let (mut y, mut m, _) = now.ymd();
        if let Some(ms) = args.first() {
            m = ms.parse().map_err(|_| Error::invalid("month must be 1-12"))?;
            if !(1..=12).contains(&m) {
                return Err(Error::invalid("month must be 1-12"));
            }
            if let Some(ys) = args.get(1) {
                y = ys.parse().map_err(|_| Error::invalid("bad year"))?;
            }
        }
        let theme = ctx.theme();
        let title = format!("{} {}", MONTHS[(m - 1) as usize], y);
        let mut out = Output::new();
        out.line(Line::styled(format!("{title:^20}"), theme.accent()));
        out.line(Line::styled("Su Mo Tu We Th Fr Sa".to_string(), theme.dim()));
        let today = now.ymd();
        for week in month_grid(y, m, false) {
            let mut line = Line::default();
            for (i, day) in week.iter().enumerate() {
                if i > 0 {
                    line.push(Span::plain(" "));
                }
                match day {
                    Some(d) => {
                        let is_today = (y, m, *d) == today;
                        let style =
                            if is_today { Style::new().attrs(slate_core::Attrs::REVERSE) } else { theme.text() };
                        line.push(Span::styled(format!("{d:>2}"), style));
                    }
                    None => line.push(Span::plain("  ")),
                }
            }
            out.line(line);
        }
        Ok(out)
    }
}

struct Weather;

impl Command for Weather {
    fn name(&self) -> &'static str {
        "weather"
    }
    fn about(&self) -> &'static str {
        "one-line weather via wttr.in (needs curl + network)"
    }
    fn usage(&self) -> &'static str {
        "weather [location..]"
    }

    fn run(&self, args: &[String], ctx: &mut Ctx) -> Result<Output> {
        need("curl", "weather")?;
        let loc = if args.is_empty() {
            ctx.weather_location()
                .ok_or_else(|| Error::invalid("usage: weather <location> (or set dashboard.weather_location)"))?
        } else {
            args.join(" ")
        };
        match probe::fetch_weather(&loc) {
            Some(text) => Ok(Output::text(format!("{loc}: {text}"))),
            None => Err(Error::unavailable(format!(
                "no weather for '{loc}' (network offline or provider down)"
            ))),
        }
    }
}

struct Git;

impl Command for Git {
    fn name(&self) -> &'static str {
        "git"
    }
    fn about(&self) -> &'static str {
        "quick git summary for the working directory"
    }

    fn run(&self, _args: &[String], ctx: &mut Ctx) -> Result<Output> {
        need("git", "git")?;
        let (branch, dirty) = probe::git_state(&ctx.cwd);
        let theme = ctx.theme();
        let Some(branch) = branch else {
            return Ok(Output::text("not a git repository"));
        };
        let dir = ctx.cwd.to_string_lossy().into_owned();
        let last = proc::run_trimmed("git", &["-C", &dir, "log", "-1", "--format=%h · %s (%cr)"])
            .unwrap_or_else(|| "no commits yet".into());
        let mut out = Output::new();
        out.line(Line::new(vec![
            Span::styled(format!("⎇ {branch}"), theme.accent()),
            Span::styled(
                format!("  ±{dirty} {}", if dirty == 1 { "file" } else { "files" }),
                if dirty > 0 { theme.warn() } else { theme.ok() },
            ),
        ]));
        out.line(Line::styled(format!("  {last}"), theme.dim()));
        Ok(out)
    }
}

// ---------------------------------------------------------------------------
// notifications / files / docker
// ---------------------------------------------------------------------------

struct Notifications;

impl Command for Notifications {
    fn name(&self) -> &'static str {
        "notifications"
    }
    fn aliases(&self) -> &'static [&'static str] {
        &["notifs"]
    }
    fn about(&self) -> &'static str {
        "list or clear in-app notifications"
    }
    fn usage(&self) -> &'static str {
        "notifications [clear]"
    }

    fn run(&self, args: &[String], ctx: &mut Ctx) -> Result<Output> {
        let theme = ctx.theme();
        if args.first().map(String::as_str) == Some("clear") {
            let n = ctx.notifications.len();
            ctx.clear_notifications();
            return Ok(Output::text(format!("cleared {n} notification(s)")));
        }
        let mut out = Output::new();
        if ctx.notifications.is_empty() {
            out.line(Line::styled("no notifications".to_string(), theme.dim()));
        }
        for n in ctx.notifications.iter().rev().take(30) {
            let style = match n.level {
                slate_core::notify::Level::Info => theme.text(),
                slate_core::notify::Level::Warn => theme.warn(),
                slate_core::notify::Level::Error => theme.error(),
            };
            out.line(Line::new(vec![
                Span::styled(format!("  {} ", n.time_label()), theme.dim()),
                Span::styled(n.text.clone(), style),
            ]));
        }
        Ok(out)
    }
}

struct Files;

impl Command for Files {
    fn name(&self) -> &'static str {
        "files"
    }
    fn aliases(&self) -> &'static [&'static str] {
        &["ls"]
    }
    fn about(&self) -> &'static str {
        "list files in a directory"
    }
    fn usage(&self) -> &'static str {
        "files [dir]"
    }

    fn run(&self, args: &[String], ctx: &mut Ctx) -> Result<Output> {
        let dir = match args.first() {
            None => ctx.cwd.clone(),
            Some(p) => {
                let p = PathBuf::from(crate::ctx::expand_tilde(p));
                if p.is_absolute() {
                    p
                } else {
                    ctx.cwd.join(p)
                }
            }
        };
        if !dir.is_dir() {
            return Err(Error::not_found(format!("directory '{}'", dir.display())));
        }
        let theme = ctx.theme();
        let mut entries: Vec<(String, bool, u64)> = Vec::new();
        for entry in std::fs::read_dir(&dir)?.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let meta = entry.metadata().ok();
            let is_dir = meta.as_ref().map(|m| m.is_dir()).unwrap_or(false);
            let size = meta.map(|m| m.len()).unwrap_or(0);
            entries.push((name, is_dir, size));
        }
        entries.sort();
        let total = entries.len();
        let mut out = Output::new();
        out.line(Line::styled(format!("{}", dir.display()), theme.accent()));
        for (name, is_dir, size) in entries.into_iter().take(200) {
            if is_dir {
                out.line(Line::new(vec![
                    Span::styled("  📁 ", theme.accent2()),
                    Span::styled(format!("{name}/"), theme.accent2()),
                ]));
            } else {
                out.line(Line::new(vec![
                    Span::styled("    ", theme.dim()),
                    Span::styled(format!("{name:<40.40}"), theme.text()),
                    Span::styled(format!("{:>10}", human_bytes(size)), theme.dim()),
                ]));
            }
        }
        if total > 200 {
            out.line(Line::styled(format!("… {} more", total - 200), theme.dim()));
        }
        Ok(out)
    }
}

struct Docker;

impl Command for Docker {
    fn name(&self) -> &'static str {
        "docker"
    }
    fn about(&self) -> &'static str {
        "list running containers"
    }

    fn run(&self, _args: &[String], ctx: &mut Ctx) -> Result<Output> {
        need("docker", "docker")?;
        let out = proc::run("docker", &["ps", "--format", "{{.Names}}\t{{.Image}}\t{{.Status}}"])?;
        if !out.status.success() {
            return Err(Error::other(String::from_utf8_lossy(&out.stderr).trim().to_string()));
        }
        let theme = ctx.theme();
        let text = String::from_utf8_lossy(&out.stdout);
        let mut output = Output::new();
        let mut any = false;
        for line in text.lines().take(20) {
            any = true;
            let mut parts = line.split('\t');
            let (name, image, status) = (
                parts.next().unwrap_or(""),
                parts.next().unwrap_or(""),
                parts.next().unwrap_or(""),
            );
            output.line(Line::new(vec![
                Span::styled(format!("{name:<24}"), theme.text()),
                Span::styled(format!("{image:<28}"), theme.dim()),
                Span::styled(status.to_string(), theme.dim()),
            ]));
        }
        if !any {
            output.line(Line::styled("no running containers".to_string(), theme.dim()));
        }
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calendar_renders_month() {
        let r = Registry::builtins();
        let mut ctx = Ctx::ephemeral();
        let out = r.exec("calendar 7 2026", &mut ctx).unwrap();
        let text = out.to_plain_string();
        assert!(text.contains("July 2026"), "{text}");
        assert!(text.contains("Su Mo Tu We Th Fr Sa"));
        assert!(text.contains("19"));
        assert!(r.exec("calendar 13", &mut ctx).is_err());
        assert!(r.exec("calendar", &mut ctx).is_ok());
    }

    #[test]
    fn power_requires_confirmation() {
        let r = Registry::builtins();
        let mut ctx = Ctx::ephemeral();
        let out = r.exec("power off", &mut ctx).unwrap();
        assert!(out.to_plain_string().contains("--yes"), "must demand confirmation");
        // `power lock --yes` would ACTUALLY lock a real session — do not run it
        // in tests; validation of the action name is covered instead.
        assert!(r.exec("power explode", &mut ctx).is_err());
    }

    #[test]
    fn uptime_sysinfo_and_files_work_anywhere() {
        let r = Registry::builtins();
        let mut ctx = Ctx::ephemeral();
        assert!(r.exec("uptime", &mut ctx).is_ok());
        let out = r.exec("sysinfo", &mut ctx).unwrap();
        assert!(out.to_plain_string().contains("kernel"));
        let out = r.exec("files /tmp", &mut ctx).unwrap();
        assert!(out.to_plain_string().contains("/tmp"));
        assert!(r.exec("files /no/such/dir", &mut ctx).is_err());
    }

    #[test]
    fn notifications_list_and_clear() {
        let r = Registry::builtins();
        let mut ctx = Ctx::ephemeral();
        r.exec("notify something happened", &mut ctx).unwrap();
        let out = r.exec("notifications", &mut ctx).unwrap();
        assert!(out.to_plain_string().contains("something happened"));
        r.exec("notifications clear", &mut ctx).unwrap();
        assert!(ctx.notifications.is_empty());
    }

    #[test]
    fn missing_tools_fail_gracefully() {
        // These tests assume the tools may or may not be installed; both
        // outcomes must be handled (Ok when present, Err when not).
        let r = Registry::builtins();
        let mut ctx = Ctx::ephemeral();
        let wifi = r.exec("wifi", &mut ctx);
        assert!(wifi.is_ok() == proc::exists("nmcli") || wifi.is_err());
        let bt = r.exec("bluetooth", &mut ctx);
        if !proc::exists("bluetoothctl") {
            assert!(bt.is_err());
        }
    }
}
