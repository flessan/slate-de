//! [`App`] — the desktop state machine.
//!
//! The app owns the [`crate::panes`], the session transcript (everything the
//! prompt printed), the search overlay, and the focus system. Commands do
//! the actual work — the app just routes keys and applies [`Effect`]s.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use slate_command::{Ctx, Output, Registry};
use slate_core::effect::Effect;
use slate_core::error::Result;
use slate_core::rect::Rect;
use slate_core::style::{Line, Span};
use slate_launcher::Item;
use slate_term::widgets::{block, BlockSpec, Borders, list, ListSpec};
use slate_term::{Buffer, Key, KeyCode};

use crate::panes::{self, Pane};

const MAX_TRANSCRIPT: usize = 4000;
const MAX_LOGS: usize = 500;

/// Which interaction layer is active.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Search,
}

/// The universal-search overlay state.
#[derive(Clone, Debug)]
pub struct SearchState {
    pub input: String,
    pub results: Vec<Item>,
    pub sel: usize,
}

/// The desktop application.
pub struct App {
    pub ctx: Ctx,
    pub registry: Registry,
    panes: Vec<Box<dyn Pane>>,
    /// Session transcript: everything ever printed to the prompt.
    pub transcript: Vec<Line>,
    /// Session event log (the `logs` pane).
    pub logs: VecDeque<Line>,
    pub focus: String,
    pub zoomed: bool,
    pub mode: Mode,
    pub search: Option<SearchState>,
    pub running: bool,
    pub dirty: bool,
    /// Cursor for the upcoming frame (applied one frame later by the driver).
    pub cursor_out: Option<(u16, u16)>,
    rects: Vec<(String, Rect)>,
    tick: Duration,
    last_frame: Instant,
    time_cache: (String, Instant),
}

impl App {
    pub fn new(ctx: Ctx, registry: Registry) -> Self {
        let tick = Duration::from_millis(u64::from(ctx.cfg.get_u16("ui.tick_ms", 250)));
        let panes: Vec<Box<dyn Pane>> = panes::default_panes(&ctx);
        let mut app = App {
            ctx,
            registry,
            panes,
            transcript: Vec::new(),
            logs: VecDeque::new(),
            focus: "prompt".to_string(),
            zoomed: false,
            mode: Mode::Normal,
            search: None,
            running: true,
            dirty: true,
            cursor_out: None,
            rects: Vec::new(),
            tick,
            last_frame: Instant::now(),
            time_cache: (String::new(), Instant::now() - Duration::from_secs(10)),
        };
        if app.ctx.cfg.get_bool("prompt.welcome", true) {
            app.seed_welcome();
        }
        app.log(format!("session started ({} panes, {} commands)", app.panes.len(), app.registry.len()));
        app
    }

    fn seed_welcome(&mut self) {
        let theme = self.ctx.theme();
        self.transcript.push(Line::styled(
            format!("slate v{} — the command line is your desktop", slate_core::VERSION),
            theme.accent(),
        ));
        self.transcript.push(Line::styled(
            "type `help` for commands · Ctrl+Space to search everything · `keys` for shortcuts"
                .to_string(),
            theme.dim(),
        ));
        self.transcript.push(Line::default());
    }

    /// How long the driver should wait between ticks.
    pub fn tick_duration(&self) -> Duration {
        self.tick
    }

    fn log(&mut self, msg: impl Into<String>) {
        let theme = self.ctx.theme();
        if self.logs.len() >= MAX_LOGS {
            self.logs.pop_front();
        }
        self.logs.push_back(Line::styled(msg.into(), theme.dim()));
        self.dirty = true;
    }

    // ------------------------------------------------------------------
    // commands and effects
    // ------------------------------------------------------------------

    /// Execute a prompt line: echo it, run it, append output, apply effects.
    /// Returns an error string when the command failed (used by drivers).
    pub fn run_command_line(&mut self, line: &str) -> String {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return String::new();
        }
        // Echo the submitted line first (owned data — no borrow conflicts).
        let symbol = self.ctx.cfg.get_str("prompt.symbol", "❯");
        let echo = Line::new(vec![
            Span::styled(format!("{symbol} "), self.ctx.theme().accent()),
            Span::styled(trimmed.to_string(), self.ctx.theme().text()),
        ]);
        self.transcript.push(echo);

        let result = self.registry.exec(trimmed, &mut self.ctx);
        let mut reply = String::new();
        match result {
            Ok(mut out) => {
                self.transcript.append(&mut out.lines);
                for effect in std::mem::take(&mut out.effects) {
                    self.apply_effect(effect);
                }
            }
            Err(e) => {
                reply = format!("error: {e}");
                let style = self.ctx.theme().error();
                self.transcript.push(Line::styled(reply.clone(), style));
                self.log(reply.clone());
            }
        }
        self.transcript.push(Line::default());
        if self.transcript.len() > MAX_TRANSCRIPT {
            let drop = self.transcript.len() - MAX_TRANSCRIPT;
            self.transcript.drain(..drop);
        }
        self.dirty = true;
        reply
    }

    /// Execute and capture the *plain text* output (for IPC/`slatectl`).
    pub fn run_command_text(&mut self, line: &str) -> (bool, String) {
        let out = self.registry.exec(line.trim(), &mut self.ctx);
        match out {
            Ok(mut o) => {
                let text = o.to_plain_string();
                let mut effects = std::mem::take(&mut o.effects);
                for effect in effects.drain(..) {
                    self.apply_effect(effect);
                }
                self.dirty = true;
                (true, text)
            }
            Err(e) => {
                self.dirty = true;
                (false, e.to_string())
            }
        }
    }

    fn apply_effect(&mut self, effect: Effect) {
        match effect {
            Effect::Quit => {
                self.running = false;
            }
            Effect::Redraw => self.dirty = true,
            Effect::SwitchWorkspace(name) => {
                let _ = self.ctx.workspaces.switch(&name);
                self.focus_first_pane();
                self.log(format!("workspace → {name}"));
            }
            Effect::NextWorkspace => {
                self.ctx.workspaces.next();
                self.focus_first_pane();
                self.log(format!("workspace → {}", self.ctx.workspaces.current_name()));
            }
            Effect::PrevWorkspace => {
                self.ctx.workspaces.prev();
                self.focus_first_pane();
                self.log(format!("workspace → {}", self.ctx.workspaces.current_name()));
            }
            Effect::SetTheme(name) => {
                self.log(format!("theme → {name}"));
                self.dirty = true;
            }
            Effect::FocusPane(name) => {
                if self.pane_exists(&name) {
                    self.focus = name;
                    self.zoomed = false;
                }
            }
            Effect::OpenSearch(query) => self.open_search(&query),
            Effect::ReloadConfig => {
                self.log("configuration reloaded");
                self.dirty = true;
            }
            Effect::Notify(text) => {
                self.log(text);
            }
            Effect::ClearScrollback => {
                self.transcript.clear();
            }
        }
    }

    fn focus_first_pane(&mut self) {
        let names = self.ctx.workspaces.current().layout.names();
        if let Some(first) = names.first() {
            self.focus = first.to_string();
        }
        self.rects.clear();
        self.dirty = true;
    }

    fn pane_exists(&self, name: &str) -> bool {
        self.ctx.workspaces.current().layout.names().iter().any(|n| *n == name)
            || self.panes.iter().any(|p| p.id() == name)
    }

    // ------------------------------------------------------------------
    // search overlay
    // ------------------------------------------------------------------

    pub fn open_search(&mut self, query: &str) {
        self.mode = Mode::Search;
        let mut state = SearchState { input: query.to_string(), results: Vec::new(), sel: 0 };
        self.update_search(&mut state);
        self.search = Some(state);
        self.dirty = true;
        self.log("search opened");
    }

    fn update_search(&mut self, state: &mut SearchState) {
        if self.ctx.search_dirty || self.ctx.index.is_empty() {
            let registry = slate_command::Registry::builtins();
            self.ctx.refresh_search(&registry);
        }
        state.results = self.ctx.index.query(&state.input, 12).into_iter().cloned().collect();
        if state.sel >= state.results.len() {
            state.sel = state.results.len().saturating_sub(1);
        }
    }

    fn close_search(&mut self) {
        self.mode = Mode::Normal;
        self.search = None;
        self.dirty = true;
    }

    fn handle_search_key(&mut self, key: Key, st: &mut SearchState) {
        match key.code {
            KeyCode::Esc => self.close_search(),
            KeyCode::Enter => {
                let action = st.results.get(st.sel).map(|i| i.action.clone());
                self.close_search();
                if let Some(action) = action {
                    let _ = self.run_command_line(&action);
                }
            }
            KeyCode::Up => {
                st.sel = st.sel.saturating_sub(1);
                self.dirty = true;
            }
            KeyCode::Down => {
                if st.sel + 1 < st.results.len() {
                    st.sel += 1;
                }
                self.dirty = true;
            }
            KeyCode::Backspace => {
                st.input.pop();
                self.update_search(st);
                self.dirty = true;
            }
            KeyCode::Char(c) if key.mods.is_none() => {
                st.input.push(c);
                self.update_search(st);
                self.dirty = true;
            }
            _ => {}
        }
    }

    // ------------------------------------------------------------------
    // keys
    // ------------------------------------------------------------------

    /// Route one key event.
    pub fn handle_key(&mut self, key: Key) -> Result<()> {
        // Search overlay captures everything while open.
        if self.mode == Mode::Search {
            let Some(st) = self.search.take() else { self.mode = Mode::Normal; return Ok(()) };
            let mut st = st;
            self.handle_search_key(key, &mut st);
            if self.mode == Mode::Search {
                self.search = Some(st);
            }
            return Ok(());
        }
        // Global bindings first.
        if key.is_ctrl('q') {
            self.running = false;
            return Ok(());
        }
        if key.is_ctrl(' ') {
            self.open_search("");
            return Ok(());
        }
        match key.code {
            KeyCode::Tab => {
                self.cycle_focus(1);
                return Ok(());
            }
            KeyCode::BackTab => {
                self.cycle_focus(-1);
                return Ok(());
            }
            KeyCode::Enter if key.mods.alt => {
                self.zoomed = !self.zoomed;
                self.dirty = true;
                return Ok(());
            }
            _ => {}
        }
        if let KeyCode::Char(c) = key.code {
            if key.mods.alt && !key.mods.ctrl {
                if c.is_ascii_digit() {
                    let idx = (c as usize) - ('1' as usize);
                    self.focus_by_index(idx);
                    return Ok(());
                }
                match c {
                    'h' => self.move_focus(Direction::Left),
                    'l' => self.move_focus(Direction::Right),
                    'k' => self.move_focus(Direction::Up),
                    'j' => self.move_focus(Direction::Down),
                    'w' => self.apply_effect(Effect::NextWorkspace),
                    'W' => self.apply_effect(Effect::PrevWorkspace),
                    _ => {
                        self.dispatch_to_focused(key)?;
                    }
                }
                return Ok(());
            }
        }
        if key.mods.alt {
            // Alt-modified navigation keys (arrows with alt held).
            match key.code {
                KeyCode::Left => {
                    self.move_focus(Direction::Left);
                    return Ok(());
                }
                KeyCode::Right => {
                    self.move_focus(Direction::Right);
                    return Ok(());
                }
                KeyCode::Up => {
                    self.move_focus(Direction::Up);
                    return Ok(());
                }
                KeyCode::Down => {
                    self.move_focus(Direction::Down);
                    return Ok(());
                }
                _ => {}
            }
        }
        self.dispatch_to_focused(key)?;
        Ok(())
    }

    fn dispatch_to_focused(&mut self, key: Key) -> Result<()> {
        let focus = self.focus.clone();
        // Temporarily move the pane out so it can borrow &mut App.
        if let Some(pos) = self.panes.iter().position(|p| p.id() == focus) {
            let mut pane = self.panes.remove(pos);
            let result = pane.handle_key(key, self);
            self.panes.insert(pos, pane);
            result?;
        }
        Ok(())
    }

    fn focus_order(&self) -> Vec<String> {
        if !self.rects.is_empty() {
            return self.rects.iter().map(|(n, _)| n.clone()).collect();
        }
        self.ctx.workspaces.current().layout.names().iter().map(ToString::to_string).collect()
    }

    fn cycle_focus(&mut self, step: i64) {
        let order = self.focus_order();
        if order.is_empty() {
            return;
        }
        let len = order.len() as i64;
        let cur = order.iter().position(|n| *n == self.focus).unwrap_or(0) as i64;
        let next = ((cur + step) % len + len) % len;
        self.focus = order[next as usize].clone();
        self.zoomed = false;
        self.dirty = true;
    }

    fn focus_by_index(&mut self, idx: usize) {
        let order = self.focus_order();
        if let Some(name) = order.get(idx) {
            self.focus = name.clone();
            self.zoomed = false;
            self.dirty = true;
        }
    }

    fn move_focus(&mut self, dir: Direction) {
        if self.rects.len() < 2 {
            self.cycle_focus(1);
            return;
        }
        fn center(r: &Rect) -> (i32, i32) {
            (i32::from(r.x) + i32::from(r.w) / 2, i32::from(r.y) + i32::from(r.h) / 2)
        }
        let Some((_, cur_rect)) = self.rects.iter().find(|(n, _)| n == &self.focus) else {
            self.cycle_focus(1);
            return;
        };
        let (cx, cy) = center(cur_rect);
        let mut best: Option<(i64, String)> = None;
        for (name, rect) in &self.rects {
            if name == &self.focus {
                continue;
            }
            let (x, y) = center(rect);
            let (dx, dy) = (x - cx, y - cy);
            let aligned = match dir {
                Direction::Left => dx < 0 && dy.abs() < (rect.h.max(1) as i32),
                Direction::Right => dx > 0 && dy.abs() < (rect.h.max(1) as i32),
                Direction::Up => dy < 0 && dx.abs() < (rect.w.max(1) as i32),
                Direction::Down => dy > 0 && dx.abs() < (rect.w.max(1) as i32),
            };
            if aligned {
                let dist = i64::from(dx * dx + dy * dy);
                if best.as_ref().map(|(d, _)| dist < *d).unwrap_or(true) {
                    best = Some((dist, name.clone()));
                }
            }
        }
        if let Some((_, name)) = best {
            self.focus = name;
            self.zoomed = false;
            self.dirty = true;
        }
    }

    // ------------------------------------------------------------------
    // ticks and drawing
    // ------------------------------------------------------------------

    /// Called by the driver every tick.
    pub fn on_tick(&mut self) {
        for i in 0..self.panes.len() {
            let mut pane = self.panes.remove(i);
            pane.on_tick(self);
            self.panes.insert(i, pane);
        }
    }

    fn pane_title(&self, name: &str) -> String {
        self.panes
            .iter()
            .find(|p| p.id() == name)
            .map(|p| p.title(&self.ctx))
            .unwrap_or_else(|| name.to_string())
    }

    fn status_time(&mut self) -> String {
        let (ref cached, ref instant) = self.time_cache;
        if instant.elapsed() < Duration::from_millis(900) {
            return cached.clone();
        }
        let text = if self.ctx.demo {
            "09:41".to_string()
        } else {
            slate_core::proc::run_trimmed("date", &["+%H:%M"])
                .unwrap_or_else(|| slate_core::civil::DateTime::now_utc().format_time())
        };
        self.time_cache = (text.clone(), Instant::now());
        text
    }

    /// Render one frame into `buf`.
    pub fn draw_frame(&mut self, buf: &mut Buffer, area: Rect) {
        // Clone the theme once per frame: drawing takes &mut self (rects,
        // focus fixups), so an immutable theme borrow could not survive.
        let theme = self.ctx.theme().clone();
        buf.fill(area, theme.text());
        let gap = self.ctx.cfg.get_u16("ui.gap", 0);
        let show_bar = self.ctx.cfg.get_u16("ui.status_bar", 1) != 0;
        let main = if show_bar {
            Rect::new(area.x, area.y + 1, area.w, area.h.saturating_sub(1))
        } else {
            area
        };
        // Layout rects (cached for focus geometry).
        let layout = self.ctx.workspaces.current().layout.clone();
        let mut rects = layout.compute(main, gap);
        if self.zoomed {
            rects.retain(|(n, _)| *n == self.focus);
            for (_, r) in &mut rects {
                *r = main;
            }
        }
        self.rects = rects.clone();
        // Ensure focus refers to a visible pane.
        if !rects.iter().any(|(n, _)| *n == self.focus) {
            if let Some((first, _)) = rects.first() {
                self.focus = first.clone();
            }
        }
        // Draw each pane.
        self.cursor_out = None;
        for (name, rect) in &rects {
            let focused = *name == self.focus;
            let title = self.pane_title(name);
            let border_style = if focused { theme.border_focused() } else { theme.border() };
            let title_text =
                format!(" {}{} ", title, if focused && self.zoomed { " [zoom]" } else { "" });
            let spec = BlockSpec {
                title: Some(&title_text),
                kind: theme.borders,
                borders: Borders::ALL,
                border_style,
                title_style: if focused { theme.border_focused() } else { theme.dim() },
                pad: 0,
            };
            let inner = block(buf, *rect, &spec);
            let mut cursor = None;
            if let Some(pos) = self.panes.iter().position(|p| p.id() == *name) {
                cursor = self.panes[pos].draw(buf, inner, focused, self);
            } else {
                // Unknown pane names from custom layouts get a placeholder.
                slate_term::widgets::paragraph(
                    buf,
                    inner,
                    &[Line::styled(
                        format!("pane '{name}' — no widget registered yet (coming in v0.4)"),
                        theme.dim(),
                    )],
                    Default::default(),
                );
            }
            if focused {
                self.cursor_out = cursor;
            }
        }
        // Status bar.
        if show_bar {
            self.draw_status_bar(buf, area);
        }
        // Search overlay on top.
        if self.mode == Mode::Search {
            let Some(st) = self.search.take() else { return };
            let mut st = st;
            self.draw_search(buf, area, &mut st);
            self.search = Some(st);
            self.cursor_out = None; // overlay manages its own cursor visibility
        }
    }

    fn draw_status_bar(&mut self, buf: &mut Buffer, area: Rect) {
        let theme = self.ctx.theme();
        let bar = Rect::new(area.x, area.y, area.w, 1);
        buf.fill(bar, theme.bar());
        let ws = self.ctx.workspaces.current_name();
        let left = format!(" slate · {ws} · {}", self.focus);
        buf.write_str(bar.x, bar.y, &left, theme.bar(), bar.w);
        let notif = self.ctx.notifications.len();
        let right = if self.ctx.demo {
            let bell = if notif > 0 { format!("●{notif} ") } else { String::new() };
            format!("{bell}{} · 09:41 ", self.ctx.themes.current_name())
        } else {
            let time = self.status_time();
            let bell = if notif > 0 { format!("●{notif} ") } else { String::new() };
            format!("{bell}{} · {time} ", self.ctx.themes.current_name())
        };
        let rw = slate_core::unicode::str_width(&right) as u16;
        if rw < bar.w {
            buf.write_str(bar.right() - rw, bar.y, &right, theme.bar(), rw);
        }
    }

    fn draw_search(&self, buf: &mut Buffer, area: Rect, st: &mut SearchState) {
        let theme = self.ctx.theme();
        let w = (area.w * 3 / 5).clamp(20, area.w.saturating_sub(2).max(20));
        let h = 14u16.min(area.h.saturating_sub(2)).max(6);
        let overlay = Rect::new(
            area.x + area.w.saturating_sub(w) / 2,
            area.y + area.h.saturating_sub(h) / 4,
            w.min(area.w),
            h.min(area.h),
        );
        buf.fill(overlay, theme.text());
        let spec = BlockSpec {
            title: Some(" search "),
            kind: theme.borders,
            borders: Borders::ALL,
            border_style: theme.border_focused(),
            title_style: theme.accent(),
            pad: 0,
        };
        let inner = block(buf, overlay, &spec);
        if inner.h == 0 {
            return;
        }
        // Input line.
        let prompt = format!("{} ", self.ctx.theme().glyph("search"));
        buf.write_str(inner.x, inner.y, &prompt, theme.accent(), inner.w);
        let text_x = inner.x + slate_core::unicode::str_width(&prompt) as u16;
        buf.write_str(text_x, inner.y, &st.input, theme.text(), inner.w.saturating_sub(2));
        // Results.
        let items: Vec<Line> = st
            .results
            .iter()
            .map(|i| {
                Line::new(vec![
                    Span::styled(format!("[{}] ", i.kind.label()), theme.accent2()),
                    Span::styled(format!("{:<24.24}", i.title), theme.text()),
                    Span::styled(slate_core::unicode::ellipsize(&i.detail, 24), theme.dim()),
                ])
            })
            .collect();
        let list_area = Rect::new(inner.x, inner.y + 2, inner.w, inner.h.saturating_sub(2));
        list(buf, list_area, &ListSpec {
            items: &items,
            selected: if items.is_empty() { None } else { Some(st.sel) },
            offset: st.sel.saturating_sub(usize::from(list_area.h) - 1),
            highlight: theme.selection(),
        });
        if items.is_empty() {
            buf.write_str(list_area.x, list_area.y, "no results", theme.dim(), list_area.w);
        }
    }
}

/// Cardinal directions for geometric focus movement (distinct from
/// `slate_core::rect::Direction`, which is a layout axis).
#[derive(Clone, Copy)]
enum Direction {
    Left,
    Right,
    Up,
    Down,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn demo_ctx() -> Ctx {
        let mut ctx = Ctx::ephemeral();
        ctx.demo = true;
        ctx.headless = false;
        ctx
    }

    #[test]
    fn app_boots_and_runs_commands() {
        let mut app = App::new(demo_ctx(), Registry::builtins());
        assert!(app.running);
        assert!(app.run_command_line("notes new test").is_empty());
        assert!(app.ctx.notes.exists("test"));
        // unknown command is captured into the transcript as an error
        let msg = app.run_command_line("nope-command");
        assert!(msg.contains("unknown command"), "{msg}");
        assert!(app.transcript.iter().any(|l| l.to_plain().contains("unknown command")));
    }

    #[test]
    fn effects_drive_the_session() {
        let mut app = App::new(demo_ctx(), Registry::builtins());
        app.run_command_line("workspace switch coding");
        assert_eq!(app.ctx.workspaces.current_name(), "coding");
        app.run_command_line("theme set slate-light");
        assert_eq!(app.ctx.themes.current_name(), "slate-light");
        app.run_command_line("quit");
        assert!(!app.running);
    }

    #[test]
    fn focus_cycling_and_geometry() {
        let mut app = App::new(demo_ctx(), Registry::builtins());
        assert_eq!(app.focus, "prompt");
        app.cycle_focus(1);
        let after = app.focus.clone();
        assert_ne!(after, "prompt");
        app.cycle_focus(-1);
        assert_eq!(app.focus, "prompt");
        app.focus_by_index(0);
        assert_eq!(app.focus, "prompt");
    }

    #[test]
    fn search_overlay_flow() {
        let mut app = App::new(demo_ctx(), Registry::builtins());
        app.open_search("theme");
        assert_eq!(app.mode, Mode::Search);
        // type more, move selection, accept
        app.handle_key(Key::plain(KeyCode::Down)).unwrap();
        app.handle_key(Key::plain(KeyCode::Enter)).unwrap();
        assert_eq!(app.mode, Mode::Normal);
        // A command from the search result was executed into the transcript.
        assert!(!app.transcript.is_empty());
    }

    #[test]
    fn draw_frame_produces_content() {
        let mut app = App::new(demo_ctx(), Registry::builtins());
        let mut term = slate_term::test_terminal(100, 30);
        term.draw(|buf, area| app.draw_frame(buf, area)).unwrap();
        let text = term.backend().buffer().to_plain_text();
        assert!(text.contains("slate"), "{text}");
        assert!(text.contains("prompt"), "{text}");
        assert!(text.contains("notes"), "{text}");
        assert!(text.contains("dashboard"), "{text}");
        assert!(text.contains("logs"), "{text}");
        assert!(text.contains("main"), "{text}"); // workspace name in status bar
    }

    #[test]
    fn zoom_and_search_render() {
        let mut app = App::new(demo_ctx(), Registry::builtins());
        app.zoomed = true;
        let mut term = slate_term::test_terminal(80, 24);
        term.draw(|buf, area| app.draw_frame(buf, area)).unwrap();
        assert!(term.backend().buffer().to_plain_text().contains("[zoom]"));

        app.zoomed = false;
        app.open_search("notes");
        term.draw(|buf, area| app.draw_frame(buf, area)).unwrap();
        let t2 = term.backend().buffer().to_plain_text();
        assert!(t2.contains("search"), "{t2}");
        assert!(t2.contains("[cmd]"), "{t2}");
    }
}
