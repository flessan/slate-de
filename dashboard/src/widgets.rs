//! The built-in widget set.
//!
//! Each widget is intentionally small: sample in `refresh`, format in
//! `render`. In demo mode every widget emits deterministic data so
//! `slate snapshot` and screenshot tests are perfectly reproducible.

use slate_core::civil::DateTime;
use slate_core::proc;
use slate_core::style::{Line, Span};
use slate_core::unicode::ellipsize;
use slate_theme::Theme;

use crate::probe::{
    human_bytes,
    human_rate,
    human_uptime,
    volume_state,
    now_playing,
    git_state,
    fetch_weather,
    Battery,
};
use crate::{Widget, WidgetEnv};

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

/// `LABEL [██████░░░░] extra` — a colored one-look status line.
fn gauge_line(theme: &Theme, label: &str, ratio: f64, extra: &str) -> Line {
    const BAR: usize = 10;
    let filled = ((ratio.clamp(0.0, 1.0) * BAR as f64).round() as usize).min(BAR);
    let color = theme.gauge_ratio_color(ratio);
    let bar: String = theme.glyph("gauge_full").repeat(filled)
        + &theme.glyph("gauge_empty").repeat(BAR - filled);
    Line::new(vec![
        Span::styled(format!("{label:<5}"), theme.dim()),
        Span::styled(format!("[{bar}]"), theme.text().fg(color)),
        Span::styled(format!(" {extra}"), theme.text()),
    ])
}

/// Spark bar glyphs for the CPU trend line (mirrors `slate-term`'s set).
const BARS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

fn bar_char(v: u64) -> char {
    if v == 0 {
        BARS[0]
    } else {
        BARS[((v.min(100) * 7) / 100) as usize]
    }
}

fn kv_line(theme: &Theme, label: &str, value: &str, style: slate_core::style::Style) -> Line {
    Line::new(vec![
        Span::styled(format!("{label:<5}"), theme.dim()),
        Span::styled(value.to_string(), style),
    ])
}

/// Local wall-clock via `date(1)` (universal on Linux), UTC fallback.
fn local_time_parts() -> (String, String, String) {
    if let Some(out) = proc::run_trimmed("date", &["+%H:%M|%Y-%m-%d|%A"]) {
        let mut it = out.split('|');
        if let (Some(t), Some(d), Some(w)) = (it.next(), it.next(), it.next()) {
            return (t.to_string(), d.to_string(), w.to_string());
        }
    }
    let now = DateTime::now_utc();
    (
        now.format_time(),
        now.format_date(),
        slate_core::civil::WEEKDAYS[now.weekday()].to_string(),
    )
}

const DEMO_CPU: [u64; 10] = [20, 35, 48, 62, 55, 44, 62, 70, 50, 62];

// ---------------------------------------------------------------------------
// clock / date / uptime / load
// ---------------------------------------------------------------------------

/// Local time (HH:MM) + date line.
pub struct Clock {
    time: String,
    date: String,
    weekday: String,
}

impl Default for Clock {
    fn default() -> Self {
        Clock { time: String::new(), date: String::new(), weekday: String::new() }
    }
}

impl Widget for Clock {
    fn name(&self) -> &'static str {
        "clock"
    }
    fn about(&self) -> &'static str {
        "local time and date"
    }
    fn interval(&self) -> u64 {
        1
    }

    fn refresh(&mut self, env: &mut WidgetEnv<'_>) {
        if env.demo {
            (self.time, self.date, self.weekday) =
                ("09:41".into(), "2026-01-05".into(), "Monday".into());
            return;
        }
        let (t, d, w) = local_time_parts();
        self.time = t;
        self.date = d;
        self.weekday = w;
    }

    fn render(&self, _width: u16, theme: &Theme) -> Vec<Line> {
        vec![
            Line::new(vec![
                Span::styled(self.time.clone(), theme.accent()),
                Span::styled(format!("  {}", self.weekday), theme.text()),
            ]),
            Line::new(vec![Span::styled(self.date.clone(), theme.dim())]),
        ]
    }
}

/// Long date for users who keep a separate date widget.
pub struct Date {
    text: String,
}

impl Default for Date {
    fn default() -> Self {
        Date { text: String::new() }
    }
}

impl Widget for Date {
    fn name(&self) -> &'static str {
        "date"
    }
    fn about(&self) -> &'static str {
        "weekday and long date"
    }
    fn interval(&self) -> u64 {
        60
    }

    fn refresh(&mut self, env: &mut WidgetEnv<'_>) {
        if env.demo {
            self.text = "Monday, 5 January 2026".into();
            return;
        }
        self.text = proc::run_trimmed("date", &["+%A, %-d %B %Y"])
            .unwrap_or_else(|| DateTime::now_utc().format_long_date());
    }

    fn render(&self, _width: u16, theme: &Theme) -> Vec<Line> {
        vec![kv_line(theme, "date", &self.text, theme.text())]
    }
}

/// System uptime.
pub struct Uptime {
    text: String,
}

impl Default for Uptime {
    fn default() -> Self {
        Uptime { text: String::new() }
    }
}

impl Widget for Uptime {
    fn name(&self) -> &'static str {
        "uptime"
    }
    fn about(&self) -> &'static str {
        "time since boot"
    }
    fn interval(&self) -> u64 {
        60
    }

    fn refresh(&mut self, env: &mut WidgetEnv<'_>) {
        self.text = if env.demo {
            "5d 3h 12m".into()
        } else {
            human_uptime(env.probe.uptime_secs())
        };
    }

    fn render(&self, _width: u16, theme: &Theme) -> Vec<Line> {
        vec![kv_line(theme, "up", &self.text, theme.text())]
    }
}

/// Load average.
pub struct LoadAvg {
    values: (f64, f64, f64),
}

impl Default for LoadAvg {
    fn default() -> Self {
        LoadAvg { values: (0.0, 0.0, 0.0) }
    }
}

impl Widget for LoadAvg {
    fn name(&self) -> &'static str {
        "load"
    }
    fn about(&self) -> &'static str {
        "load average (1m 5m 15m)"
    }
    fn interval(&self) -> u64 {
        5
    }

    fn refresh(&mut self, env: &mut WidgetEnv<'_>) {
        self.values = if env.demo { (0.42, 0.30, 0.25) } else { env.probe.loadavg() };
    }

    fn render(&self, _width: u16, theme: &Theme) -> Vec<Line> {
        let (a, b, c) = self.values;
        vec![kv_line(theme, "load", &format!("{a:.2} {b:.2} {c:.2}"), theme.text())]
    }
}

// ---------------------------------------------------------------------------
// cpu / memory / swap / disk
// ---------------------------------------------------------------------------

/// CPU usage with a rolling sparkline.
pub struct Cpu {
    frac: f64,
    history: Vec<u64>,
    tick: usize,
}

impl Default for Cpu {
    fn default() -> Self {
        Cpu { frac: 0.0, history: Vec::new(), tick: 0 }
    }
}

impl Widget for Cpu {
    fn name(&self) -> &'static str {
        "cpu"
    }
    fn about(&self) -> &'static str {
        "CPU usage gauge + trend"
    }
    fn interval(&self) -> u64 {
        2
    }

    fn refresh(&mut self, env: &mut WidgetEnv<'_>) {
        if env.demo {
            self.frac = DEMO_CPU[self.tick % DEMO_CPU.len()] as f64 / 100.0;
            self.tick += 1;
        } else {
            self.frac = env.probe.cpu_fraction();
        }
        self.history.push((self.frac * 100.0) as u64);
        if self.history.len() > 120 {
            self.history.drain(..self.history.len() - 120);
        }
    }

    fn render(&self, width: u16, theme: &Theme) -> Vec<Line> {
        let mut lines = vec![gauge_line(theme, "cpu", self.frac, &format!("{:.0}%", self.frac * 100.0))];
        if self.history.len() > 1 && width > 8 {
            let take = usize::from(width.saturating_sub(6));
            let tail: &[u64] = if self.history.len() > take {
                &self.history[self.history.len() - take..]
            } else {
                &self.history
            };
            let bars: String = tail.iter().map(|&v| bar_char(v)).collect();
            lines.push(Line::new(vec![
                Span::styled("      ", theme.dim()),
                Span::styled(bars, theme.dim()),
            ]));
        }
        lines
    }
}

/// RAM usage.
pub struct Memory {
    used: u64,
    total: u64,
}

impl Default for Memory {
    fn default() -> Self {
        Memory { used: 0, total: 0 }
    }
}

impl Widget for Memory {
    fn name(&self) -> &'static str {
        "memory"
    }
    fn about(&self) -> &'static str {
        "RAM usage"
    }
    fn interval(&self) -> u64 {
        3
    }

    fn refresh(&mut self, env: &mut WidgetEnv<'_>) {
        (self.used, self.total) = if env.demo {
            (6_900_000_000, 17_179_869_184)
        } else {
            env.probe.memory()
        };
    }

    fn render(&self, _width: u16, theme: &Theme) -> Vec<Line> {
        let ratio =
            if self.total > 0 { self.used as f64 / self.total as f64 } else { 0.0 };
        vec![gauge_line(
            theme,
            "mem",
            ratio,
            &format!("{}/{}", human_bytes(self.used), human_bytes(self.total)),
        )]
    }
}

/// Swap usage (hidden when no swap is configured).
pub struct Swap {
    used: u64,
    total: u64,
}

impl Default for Swap {
    fn default() -> Self {
        Swap { used: 0, total: 0 }
    }
}

impl Widget for Swap {
    fn name(&self) -> &'static str {
        "swap"
    }
    fn about(&self) -> &'static str {
        "swap usage"
    }
    fn interval(&self) -> u64 {
        5
    }

    fn refresh(&mut self, env: &mut WidgetEnv<'_>) {
        (self.used, self.total) =
            if env.demo { (512 * 1024 * 1024, 2 * 1024 * 1024 * 1024) } else { env.probe.swap() };
    }

    fn render(&self, _width: u16, theme: &Theme) -> Vec<Line> {
        if self.total == 0 {
            return Vec::new();
        }
        let ratio = self.used as f64 / self.total as f64;
        vec![gauge_line(
            theme,
            "swap",
            ratio,
            &format!("{}/{}", human_bytes(self.used), human_bytes(self.total)),
        )]
    }
}

/// Disk usage of `/`.
pub struct Disk {
    used: u64,
    total: u64,
}

impl Default for Disk {
    fn default() -> Self {
        Disk { used: 0, total: 0 }
    }
}

impl Widget for Disk {
    fn name(&self) -> &'static str {
        "disk"
    }
    fn about(&self) -> &'static str {
        "disk usage of /"
    }
    fn interval(&self) -> u64 {
        60
    }

    fn refresh(&mut self, env: &mut WidgetEnv<'_>) {
        (self.used, self.total) = if env.demo {
            (120_000_000_000, 512_000_000_000)
        } else {
            env.probe.disk_root()
        };
    }

    fn render(&self, _width: u16, theme: &Theme) -> Vec<Line> {
        let ratio =
            if self.total > 0 { self.used as f64 / self.total as f64 } else { 0.0 };
        vec![gauge_line(
            theme,
            "disk",
            ratio,
            &format!("{}/{}", human_bytes(self.used), human_bytes(self.total)),
        )]
    }
}

// ---------------------------------------------------------------------------
// battery / network / vpn / volume
// ---------------------------------------------------------------------------

/// Battery level (hidden on desktops without a battery).
pub struct BatteryWidget {
    state: Option<Battery>,
}

impl Default for BatteryWidget {
    fn default() -> Self {
        BatteryWidget { state: None }
    }
}

impl Widget for BatteryWidget {
    fn name(&self) -> &'static str {
        "battery"
    }
    fn about(&self) -> &'static str {
        "battery level and status"
    }
    fn interval(&self) -> u64 {
        30
    }

    fn refresh(&mut self, env: &mut WidgetEnv<'_>) {
        self.state = if env.demo {
            Some(Battery { name: "BAT0".into(), percent: 87, status: "Discharging".into() })
        } else {
            env.probe.battery()
        };
    }

    fn render(&self, _width: u16, theme: &Theme) -> Vec<Line> {
        let Some(b) = &self.state else { return Vec::new() };
        let ratio = f64::from(b.percent) / 100.0;
        vec![gauge_line(theme, "bat", ratio, &format!("{}% {}", b.percent, b.status.to_lowercase()))]
    }
}

/// Aggregate network transfer rates.
pub struct Network {
    down: u64,
    up: u64,
}

impl Default for Network {
    fn default() -> Self {
        Network { down: 0, up: 0 }
    }
}

impl Widget for Network {
    fn name(&self) -> &'static str {
        "network"
    }
    fn about(&self) -> &'static str {
        "aggregated transfer rates"
    }
    fn interval(&self) -> u64 {
        3
    }

    fn refresh(&mut self, env: &mut WidgetEnv<'_>) {
        (self.down, self.up) =
            if env.demo { (1_258_291, 327_680) } else { env.probe.net_rates() };
    }

    fn render(&self, _width: u16, theme: &Theme) -> Vec<Line> {
        vec![Line::new(vec![
            Span::styled("net  ", theme.dim()),
            Span::styled(format!("{} {:>10}", theme.glyph("dn"), human_rate(self.down)), theme.text()),
            Span::styled("  ", theme.dim()),
            Span::styled(format!("{} {:>10}", theme.glyph("up"), human_rate(self.up)), theme.text()),
        ])]
    }
}

/// Active VPN interface indicator.
pub struct Vpn {
    name: Option<String>,
}

impl Default for Vpn {
    fn default() -> Self {
        Vpn { name: None }
    }
}

impl Widget for Vpn {
    fn name(&self) -> &'static str {
        "vpn"
    }
    fn about(&self) -> &'static str {
        "active VPN interface"
    }
    fn interval(&self) -> u64 {
        10
    }

    fn refresh(&mut self, env: &mut WidgetEnv<'_>) {
        if env.demo {
            self.name = Some("wg0".into());
            return;
        }
        self.name = env.probe.active_vpn();
    }

    fn render(&self, _width: u16, theme: &Theme) -> Vec<Line> {
        match &self.name {
            Some(n) => vec![kv_line(theme, "vpn", n, theme.ok())],
            None => vec![kv_line(theme, "vpn", "off", theme.dim())],
        }
    }
}

/// Output volume via PipeWire (`wpctl`).
pub struct Volume {
    pct: Option<u8>,
    muted: bool,
    available: bool,
}

impl Default for Volume {
    fn default() -> Self {
        Volume { pct: None, muted: false, available: true }
    }
}

impl Widget for Volume {
    fn name(&self) -> &'static str {
        "volume"
    }
    fn about(&self) -> &'static str {
        "PipeWire output volume"
    }
    fn interval(&self) -> u64 {
        3
    }

    fn refresh(&mut self, env: &mut WidgetEnv<'_>) {
        if env.demo {
            (self.pct, self.muted, self.available) = (Some(42), false, true);
            return;
        }
        match volume_state() {
            Some((pct, muted)) => {
                self.pct = Some(pct);
                self.muted = muted;
                self.available = true;
            }
            None => self.available = false,
        }
    }

    fn render(&self, _width: u16, theme: &Theme) -> Vec<Line> {
        if !self.available {
            return vec![kv_line(theme, "vol", "n/a", theme.dim())];
        }
        let pct = self.pct.unwrap_or(0);
        if self.muted {
            return vec![kv_line(theme, "vol", "muted", theme.error())];
        }
        vec![gauge_line(theme, "vol", f64::from(pct) / 100.0, &format!("{pct}%"))]
    }
}

// ---------------------------------------------------------------------------
// music / git / docker / weather
// ---------------------------------------------------------------------------

/// Now-playing via `playerctl`.
pub struct Music {
    track: Option<String>,
}

impl Default for Music {
    fn default() -> Self {
        Music { track: None }
    }
}

impl Widget for Music {
    fn name(&self) -> &'static str {
        "music"
    }
    fn about(&self) -> &'static str {
        "now playing via playerctl"
    }
    fn interval(&self) -> u64 {
        5
    }

    fn refresh(&mut self, env: &mut WidgetEnv<'_>) {
        if env.demo {
            self.track = Some("Bonobo — Cirrus".into());
            return;
        }
        self.track = now_playing();
    }

    fn render(&self, width: u16, theme: &Theme) -> Vec<Line> {
        match &self.track {
            Some(t) => {
                let max = usize::from(width.saturating_sub(4));
                vec![Line::new(vec![
                    Span::styled(format!("{} ", theme.glyph("music")), theme.accent2()),
                    Span::styled(ellipsize(t, max), theme.text()),
                ])]
            }
            None => vec![kv_line(theme, "music", "stopped", theme.dim())],
        }
    }
}

/// Git branch and dirty-file count for the current directory.
pub struct Git {
    branch: Option<String>,
    dirty: usize,
}

impl Default for Git {
    fn default() -> Self {
        Git { branch: None, dirty: 0 }
    }
}

impl Widget for Git {
    fn name(&self) -> &'static str {
        "git"
    }
    fn about(&self) -> &'static str {
        "branch + dirty file count of $PWD"
    }
    fn interval(&self) -> u64 {
        10
    }

    fn refresh(&mut self, env: &mut WidgetEnv<'_>) {
        if env.demo {
            (self.branch, self.dirty) = (Some("main".into()), 2);
            return;
        }
        (self.branch, self.dirty) = git_state(env.cwd);
    }

    fn render(&self, _width: u16, theme: &Theme) -> Vec<Line> {
        match &self.branch {
            Some(b) => {
                let dirty_style =
                    if self.dirty > 0 { theme.warn() } else { theme.dim() };
                vec![Line::new(vec![
                    Span::styled("git  ", theme.dim()),
                    Span::styled(b.clone(), theme.accent()),
                    Span::styled(format!("  ±{}", self.dirty), dirty_style),
                ])]
            }
            None => vec![kv_line(theme, "git", "not a repo", theme.dim())],
        }
    }
}

/// Running Docker containers.
pub struct Docker {
    count: Option<usize>,
}

impl Default for Docker {
    fn default() -> Self {
        Docker { count: None }
    }
}

impl Widget for Docker {
    fn name(&self) -> &'static str {
        "docker"
    }
    fn about(&self) -> &'static str {
        "running container count"
    }
    fn interval(&self) -> u64 {
        30
    }

    fn refresh(&mut self, env: &mut WidgetEnv<'_>) {
        if env.demo {
            self.count = Some(3);
            return;
        }
        self.count = proc::run_trimmed("docker", &["ps", "-q"])
            .map(|s| s.lines().filter(|l| !l.trim().is_empty()).count());
    }

    fn render(&self, _width: u16, theme: &Theme) -> Vec<Line> {
        match self.count {
            Some(n) => vec![kv_line(theme, "dock", &format!("{n} running"), theme.text())],
            None => vec![kv_line(theme, "dock", "n/a", theme.dim())],
        }
    }
}

/// Weather via wttr.in (opt-in; requires `curl` + network).
pub struct Weather {
    text: Option<String>,
    configured: bool,
}

impl Default for Weather {
    fn default() -> Self {
        Weather { text: None, configured: false }
    }
}

impl Widget for Weather {
    fn name(&self) -> &'static str {
        "weather"
    }
    fn about(&self) -> &'static str {
        "wttr.in one-liner (set dashboard.weather_location)"
    }
    fn interval(&self) -> u64 {
        900
    }

    fn refresh(&mut self, env: &mut WidgetEnv<'_>) {
        if env.demo {
            self.text = Some("☀ +12°C".into());
            self.configured = true;
            return;
        }
        self.configured = env.weather_location.is_some();
        self.text = env.weather_location.and_then(fetch_weather);
    }

    fn render(&self, _width: u16, theme: &Theme) -> Vec<Line> {
        if let Some(t) = &self.text {
            vec![kv_line(theme, "sky", t, theme.text())]
        } else if self.configured {
            vec![kv_line(theme, "sky", "n/a", theme.dim())]
        } else {
            vec![kv_line(theme, "sky", "unset location", theme.dim())]
        }
    }
}

// ---------------------------------------------------------------------------
// workspace / notifications
// ---------------------------------------------------------------------------

/// Current workspace indicator.
pub struct WorkspaceWidget {
    name: String,
}

impl Default for WorkspaceWidget {
    fn default() -> Self {
        WorkspaceWidget { name: "main".into() }
    }
}

impl Widget for WorkspaceWidget {
    fn name(&self) -> &'static str {
        "workspace"
    }
    fn about(&self) -> &'static str {
        "active workspace name"
    }
    fn interval(&self) -> u64 {
        1
    }

    fn refresh(&mut self, env: &mut WidgetEnv<'_>) {
        self.name = env.workspace.to_string();
    }

    fn render(&self, _width: u16, theme: &Theme) -> Vec<Line> {
        vec![Line::new(vec![
            Span::styled("ws   ", theme.dim()),
            Span::styled(self.name.clone(), theme.accent()),
        ])]
    }
}

/// Notification count (hidden at zero).
pub struct Notifications {
    count: usize,
}

impl Default for Notifications {
    fn default() -> Self {
        Notifications { count: 0 }
    }
}

impl Widget for Notifications {
    fn name(&self) -> &'static str {
        "notifications"
    }
    fn about(&self) -> &'static str {
        "unread notification count"
    }
    fn interval(&self) -> u64 {
        1
    }

    fn refresh(&mut self, env: &mut WidgetEnv<'_>) {
        self.count = env.notifications;
    }

    fn render(&self, _width: u16, theme: &Theme) -> Vec<Line> {
        if self.count == 0 {
            return Vec::new();
        }
        vec![Line::new(vec![
            Span::styled(format!("{} ", theme.glyph("bell")), theme.warn()),
            Span::styled(format!("{} notification(s)", self.count), theme.text()),
        ])]
    }
}

// ---------------------------------------------------------------------------
// registry
// ---------------------------------------------------------------------------

/// `(name, about)` for every built-in widget (`dashboard widgets`).
pub fn all_widgets() -> &'static [(&'static str, &'static str)] {
    &[
        ("clock", "local time and date"),
        ("date", "weekday and long date"),
        ("uptime", "time since boot"),
        ("load", "load average"),
        ("cpu", "CPU gauge + trend"),
        ("memory", "RAM usage"),
        ("swap", "swap usage"),
        ("disk", "disk usage of /"),
        ("battery", "battery level"),
        ("network", "transfer rates"),
        ("vpn", "active VPN interface"),
        ("volume", "output volume"),
        ("music", "now playing"),
        ("git", "branch + dirty count of $PWD"),
        ("docker", "running containers"),
        ("weather", "wttr.in one-liner"),
        ("workspace", "active workspace name"),
        ("notifications", "unread notification count"),
    ]
}

/// Widget factory by registry name.
pub fn widget_by_name(name: &str) -> Option<Box<dyn Widget>> {
    match name {
        "clock" => Some(Box::new(Clock::default())),
        "date" => Some(Box::new(Date::default())),
        "uptime" => Some(Box::new(Uptime::default())),
        "load" => Some(Box::new(LoadAvg::default())),
        "cpu" => Some(Box::new(Cpu::default())),
        "memory" => Some(Box::new(Memory::default())),
        "swap" => Some(Box::new(Swap::default())),
        "disk" => Some(Box::new(Disk::default())),
        "battery" => Some(Box::new(BatteryWidget::default())),
        "network" => Some(Box::new(Network::default())),
        "vpn" => Some(Box::new(Vpn::default())),
        "volume" => Some(Box::new(Volume::default())),
        "music" => Some(Box::new(Music::default())),
        "git" => Some(Box::new(Git::default())),
        "docker" => Some(Box::new(Docker::default())),
        "weather" => Some(Box::new(Weather::default())),
        "workspace" => Some(Box::new(WorkspaceWidget::default())),
        "notifications" => Some(Box::new(Notifications::default())),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::WidgetEnv;
    use slate_theme::ThemeManager;

    fn theme() -> Theme {
        ThemeManager::builtins().get("slate-dark").cloned().unwrap_or_default()
    }

    #[test]
    fn all_widgets_refresh_and_render_in_demo() {
        let mut probe = Probe::new();
        let t = theme();
        for (name, _) in all_widgets() {
            let mut w = widget_by_name(name).unwrap();
            let mut env = WidgetEnv::demo(&mut probe);
            w.refresh(&mut env);
            w.refresh(&mut env); // exercise stateful widgets twice
            let lines = w.render(40, &t);
            let text: String = lines.iter().map(Line::to_plain).collect();
            assert!(!text.trim().is_empty(), "widget '{name}' rendered nothing in demo mode");
        }
    }

    #[test]
    fn cpu_history_grows() {
        let mut probe = Probe::new();
        let mut w = Cpu::default();
        let mut env = WidgetEnv::demo(&mut probe);
        assert_eq!(w.interval(), 2);
        for _ in 0..3 {
            w.refresh(&mut env);
        }
        assert_eq!(w.history.len(), 3);
        let lines = w.render(40, &theme());
        assert_eq!(lines.len(), 2); // gauge + sparkline
    }

    #[test]
    fn volume_states() {
        let t = theme();
        let mut w = Volume::default();
        let mut probe = Probe::new();
        let mut env = WidgetEnv::demo(&mut probe);
        w.refresh(&mut env);
        let lines = w.render(40, &t);
        assert!(lines.iter().map(Line::to_plain).collect::<String>().contains("42%"));
        w.pct = None;
        w.available = false;
        let lines = w.render(40, &t);
        assert!(lines.iter().map(Line::to_plain).collect::<String>().contains("n/a"));
    }

    #[test]
    fn git_not_a_repo_renders_dim_hint() {
        let mut w = Git::default();
        let t = theme();
        let lines = w.render(40, &t);
        assert!(lines.iter().map(Line::to_plain).collect::<String>().contains("not a repo"));
    }
}
