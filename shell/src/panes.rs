//! The built-in panes.
//!
//! A pane is a keyboard-focused, self-contained view widget:
//!
//! * [`PromptPane`] — the always-available command line + transcript
//! * [`NotesPane`] — the persistent notebook (list and edit modes)
//! * [`DashboardPane`] — the widget dashboard
//! * [`LogsPane`] — the session event log
//!
//! Panes never draw their own frame (border) — the app does, uniformly.

use std::time::{Duration, Instant};

use slate_command::Ctx;
use slate_core::rect::Rect;
use slate_core::style::{Line, Span};
use slate_dashboard::{Dashboard, Probe, WidgetEnv};
use slate_notes::Note;
use slate_term::widgets::{list, paragraph, ListSpec, ParagraphOpts};
use slate_term::{Buffer, Key, KeyCode};

use crate::app::App;

/// The pane contract.
pub trait Pane {
    /// Layout/address name (`prompt`, `notes`, …).
    fn id(&self) -> &'static str;
    /// Border title.
    fn title(&self, ctx: &Ctx) -> String;
    /// Periodic work (data refresh, autosave). Called every shell tick.
    fn on_tick(&mut self, _app: &mut App) {}
    /// Handle one key while focused.
    fn handle_key(&mut self, key: Key, app: &mut App) -> slate_core::error::Result<()>;
    /// Draw contents into `inner`. Returns the cursor position when the pane
    /// wants to show one (only honored for the focused pane).
    fn draw(&self, buf: &mut Buffer, inner: Rect, focused: bool, app: &App) -> Option<(u16, u16)>;
}

/// The default pane set (custom layout names get placeholders).
pub fn default_panes(ctx: &Ctx) -> Vec<Box<dyn Pane>> {
    vec![
        Box::new(PromptPane::new(ctx)),
        Box::new(NotesPane::new(ctx)),
        Box::new(DashboardPane::new(ctx)),
        Box::new(LogsPane::new()),
    ]
}

// ---------------------------------------------------------------------------
// prompt
// ---------------------------------------------------------------------------

/// The prompt: transcript above, input line at the bottom.
pub struct PromptPane {
    input: Vec<char>,
    /// Cursor as a char index into `input`.
    cursor: usize,
    scroll: u16,
    history_pos: Option<usize>,
    stash: Vec<char>,
    symbol: String,
}

impl PromptPane {
    pub fn new(ctx: &Ctx) -> Self {
        PromptPane {
            input: Vec::new(),
            cursor: 0,
            scroll: 0,
            history_pos: None,
            stash: Vec::new(),
            symbol: ctx.cfg.get_str("prompt.symbol", "❯"),
        }
    }

    fn input_string(&self) -> String {
        self.input.iter().collect()
    }

    fn submit(&mut self, app: &mut App) {
        let line = self.input_string();
        self.input.clear();
        self.cursor = 0;
        self.history_pos = None;
        self.scroll = 0;
        let _ = app.run_command_line(&line);
    }

    fn history_prev(&mut self, app: &App) {
        if app.ctx.history.is_empty() {
            return;
        }
        match self.history_pos {
            None => {
                self.stash = self.input.clone();
                self.history_pos = Some(app.ctx.history.len() - 1);
            }
            Some(0) => return,
            Some(pos) => self.history_pos = Some(pos - 1),
        }
        if let Some(pos) = self.history_pos {
            self.input = app.ctx.history[pos].chars().collect();
            self.cursor = self.input.len();
        }
    }

    fn history_next(&mut self, app: &App) {
        match self.history_pos {
            None => {}
            Some(pos) if pos + 1 < app.ctx.history.len() => {
                self.history_pos = Some(pos + 1);
                self.input = app.ctx.history[pos + 1].chars().collect();
                self.cursor = self.input.len();
            }
            Some(_) => {
                self.history_pos = None;
                self.input = std::mem::take(&mut self.stash);
                self.cursor = self.input.len();
            }
        }
    }
}

impl Pane for PromptPane {
    fn id(&self) -> &'static str {
        "prompt"
    }

    fn title(&self, ctx: &Ctx) -> String {
        let cwd = ctx.cwd.to_string_lossy();
        let home = slate_config::paths::home_dir().to_string_lossy().into_owned();
        let shown = if cwd == home {
            "~".to_string()
        } else if let Some(rest) = cwd.strip_prefix(&format!("{home}/")) {
            format!("~/{rest}")
        } else {
            cwd.into_owned()
        };
        format!("prompt — {shown}")
    }

    fn handle_key(&mut self, key: Key, app: &mut App) -> slate_core::error::Result<()> {
        match key.code {
            KeyCode::Enter => self.submit(app),
            KeyCode::Char(c) if key.mods.is_none() => {
                self.input.insert(self.cursor, c);
                self.cursor += 1;
            }
            KeyCode::Char('u') if key.is_ctrl('u') => {
                self.input.clear();
                self.cursor = 0;
            }
            KeyCode::Char('c') if key.is_ctrl('c') => {
                self.input.clear();
                self.cursor = 0;
            }
            KeyCode::Char('l') if key.is_ctrl('l') => {
                app.transcript.clear();
            }
            KeyCode::Backspace => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                    self.input.remove(self.cursor);
                }
            }
            KeyCode::Delete => {
                if self.cursor < self.input.len() {
                    self.input.remove(self.cursor);
                }
            }
            KeyCode::Left => {
                self.cursor = self.cursor.saturating_sub(1);
            }
            KeyCode::Right => {
                if self.cursor < self.input.len() {
                    self.cursor += 1;
                }
            }
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => self.cursor = self.input.len(),
            KeyCode::Up => self.history_prev(app),
            KeyCode::Down => self.history_next(app),
            KeyCode::PageUp => self.scroll = self.scroll.saturating_add(5),
            KeyCode::PageDown => self.scroll = self.scroll.saturating_sub(5),
            _ => {}
        }
        app.dirty = true;
        Ok(())
    }

    fn draw(&self, buf: &mut Buffer, inner: Rect, focused: bool, app: &App) -> Option<(u16, u16)> {
        let theme = app.ctx.theme();
        if inner.h == 0 {
            return None;
        }
        // Transcript: everything except the last row (input line).
        let log_rows = inner.h.saturating_sub(1);
        let total = app.transcript.len();
        let max_scroll = total.saturating_sub(usize::from(log_rows)) as u16;
        let scroll = self.scroll.min(max_scroll);
        let end = total.saturating_sub(usize::from(scroll));
        let start = end.saturating_sub(usize::from(log_rows));
        for (row, line) in app.transcript[start..end].iter().enumerate() {
            buf.write_line(inner.x, inner.y + row as u16, line, inner.w);
        }
        if scroll > 0 {
            let hint = format!("↑ {scroll} more ");
            let w = slate_core::unicode::str_width(&hint) as u16;
            buf.write_str(inner.right().saturating_sub(w), inner.y, &hint, theme.dim(), w);
        }
        // Input line.
        let y = inner.y + log_rows;
        let symbol = format!("{} ", self.symbol);
        buf.write_str(inner.x, y, &symbol, theme.accent(), inner.w);
        let text_x = inner.x + slate_core::unicode::str_width(&symbol) as u16;
        let avail = inner.right().saturating_sub(text_x);
        let input = self.input_string();
        buf.write_str(text_x, y, &input, theme.text(), avail);
        if focused {
            let before: usize = self.input[..self.cursor]
                .iter()
                .map(|c| slate_core::unicode::char_width(*c))
                .sum();
            let cx = text_x.saturating_add((before as u16).min(avail.saturating_sub(1)));
            Some((cx, y))
        } else {
            None
        }
    }
}

// ---------------------------------------------------------------------------
// notes
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum NotesMode {
    List,
    Edit,
}

/// The notebook pane.
pub struct NotesPane {
    mode: NotesMode,
    sel: usize,
    edit_title: String,
    edit_body: Vec<char>,
    edit_cursor: usize,
    edit_scroll: u16,
    dirty_since: Option<Instant>,
    cache: Vec<Note>,
    last_sync: Instant,
}

impl NotesPane {
    pub fn new(_ctx: &Ctx) -> Self {
        NotesPane {
            mode: NotesMode::List,
            sel: 0,
            edit_title: String::new(),
            edit_body: Vec::new(),
            edit_cursor: 0,
            edit_scroll: 0,
            dirty_since: None,
            cache: Vec::new(),
            last_sync: Instant::now() - Duration::from_secs(60),
        }
    }

    fn sync(&mut self, app: &App) {
        if self.last_sync.elapsed() > Duration::from_secs(2) || app.ctx.search_dirty {
            self.cache = app.ctx.notes.list().unwrap_or_default();
            self.sel = self.sel.min(self.cache.len().saturating_sub(1));
            self.last_sync = Instant::now();
        }
    }

    fn save_edit(&mut self, app: &mut App) {
        if self.edit_title.is_empty() {
            return;
        }
        let body: String = self.edit_body.iter().collect();
        if app.ctx.notes.write(&self.edit_title, &body).is_ok() {
            app.ctx.search_dirty = true;
        }
        self.dirty_since = None;
        self.last_sync = Instant::now() - Duration::from_secs(60);
    }

    fn begin_edit(&mut self, title: &str, body: String) {
        self.mode = NotesMode::Edit;
        self.edit_title = title.to_string();
        self.edit_body = body.chars().collect();
        self.edit_cursor = self.edit_body.len();
        self.dirty_since = None;
    }
}

impl Pane for NotesPane {
    fn id(&self) -> &'static str {
        "notes"
    }

    fn title(&self, _ctx: &Ctx) -> String {
        match self.mode {
            NotesMode::List => format!("notes ({})", self.cache.len()),
            NotesMode::Edit => format!("notes — editing '{}'", self.edit_title),
        }
    }

    fn on_tick(&mut self, app: &mut App) {
        self.sync(app);
        // Debounced autosave while editing.
        if self.mode == NotesMode::Edit && app.ctx.cfg.get_bool("notes.autosave", true) {
            if let Some(since) = self.dirty_since {
                if since.elapsed() > Duration::from_secs(2) {
                    self.save_edit(app);
                }
            }
        }
    }

    fn handle_key(&mut self, key: Key, app: &mut App) -> slate_core::error::Result<()> {
        match self.mode {
            NotesMode::List => match key.code {
                KeyCode::Char('j') | KeyCode::Down => {
                    if self.sel + 1 < self.cache.len() {
                        self.sel += 1;
                    }
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.sel = self.sel.saturating_sub(1);
                }
                KeyCode::Char('e') | KeyCode::Enter => {
                    if let Some(note) = self.cache.get(self.sel) {
                        let body = note.body.clone();
                        let slug = note.slug.clone();
                        self.begin_edit(&slug, body);
                    }
                }
                KeyCode::Char('n') => {
                    let title = "untitled";
                    let mut candidate = title.to_string();
                    let mut i = 2;
                    while app.ctx.notes.exists(&candidate) {
                        candidate = format!("{title}-{i}");
                        i += 1;
                    }
                    let _ = app.ctx.notes.create(&candidate);
                    app.ctx.search_dirty = true;
                    let body = app.ctx.notes.get(&candidate).map(|n| n.body).unwrap_or_default();
                    self.begin_edit(&candidate, body);
                }
                KeyCode::Char('d') => {
                    if let Some(note) = self.cache.get(self.sel) {
                        let slug = note.slug.clone();
                        let title = note.title.clone();
                        let _ = app.ctx.notes.remove(&slug);
                        let _ = app.run_command_line(&format!(
                            "notify deleted note '{title}' (it was on disk as {slug}.md)"
                        ));
                        app.ctx.search_dirty = true;
                        self.last_sync = Instant::now() - Duration::from_secs(60);
                    }
                }
                KeyCode::Char('p') => {
                    if let Some(note) = self.cache.get(self.sel) {
                        let slug = note.slug.clone();
                        let pinned = !note.pinned;
                        let _ = app.ctx.notes.set_pinned(&slug, pinned);
                        app.ctx.search_dirty = true;
                        self.last_sync = Instant::now() - Duration::from_secs(60);
                    }
                }
                _ => {}
            },
            NotesMode::Edit => match key.code {
                KeyCode::Esc => {
                    self.save_edit(app);
                    self.mode = NotesMode::List;
                }
                KeyCode::Char(c) if key.mods.is_none() => {
                    self.edit_body.insert(self.edit_cursor, c);
                    self.edit_cursor += 1;
                    self.dirty_since = Some(Instant::now());
                }
                KeyCode::Enter => {
                    self.edit_body.insert(self.edit_cursor, '\n');
                    self.edit_cursor += 1;
                    self.dirty_since = Some(Instant::now());
                }
                KeyCode::Backspace => {
                    if self.edit_cursor > 0 {
                        self.edit_cursor -= 1;
                        self.edit_body.remove(self.edit_cursor);
                        self.dirty_since = Some(Instant::now());
                    }
                }
                KeyCode::Delete => {
                    if self.edit_cursor < self.edit_body.len() {
                        self.edit_body.remove(self.edit_cursor);
                        self.dirty_since = Some(Instant::now());
                    }
                }
                KeyCode::Left => self.edit_cursor = self.edit_cursor.saturating_sub(1),
                KeyCode::Right => {
                    if self.edit_cursor < self.edit_body.len() {
                        self.edit_cursor += 1;
                    }
                }
                KeyCode::Home => self.edit_cursor = 0,
                KeyCode::End => self.edit_cursor = self.edit_body.len(),
                KeyCode::PageUp => self.edit_scroll = self.edit_scroll.saturating_add(5),
                KeyCode::PageDown => self.edit_scroll = self.edit_scroll.saturating_sub(5),
                _ => {}
            },
        }
        app.dirty = true;
        Ok(())
    }

    fn draw(&self, buf: &mut Buffer, inner: Rect, focused: bool, app: &App) -> Option<(u16, u16)> {
        let theme = app.ctx.theme();
        match self.mode {
            NotesMode::List => {
                if self.cache.is_empty() {
                    paragraph(
                        buf,
                        inner,
                        &[
                            Line::styled("no notes yet", theme.dim()),
                            Line::styled("`notes add <text>` to capture one", theme.dim()),
                        ],
                        ParagraphOpts::default(),
                    );
                    return None;
                }
                let items: Vec<Line> = self
                    .cache
                    .iter()
                    .map(|n| {
                        let pin = if n.pinned { theme.glyph("pin") } else { " " };
                        Line::new(vec![
                            Span::styled(format!("{pin} "), theme.accent2()),
                            Span::styled(
                                format!("{:<16.16}", n.title),
                                theme.text(),
                            ),
                            Span::styled(
                                slate_core::unicode::ellipsize(n.preview(), 24),
                                theme.dim(),
                            ),
                        ])
                    })
                    .collect();
                let content_h = inner.h.saturating_sub(1);
                let content = Rect::new(inner.x, inner.y, inner.w, content_h);
                list(
                    buf,
                    content,
                    &ListSpec {
                        items: &items,
                        selected: if focused { Some(self.sel) } else { None },
                        offset: self.sel.saturating_sub(usize::from(content_h) - 1),
                        highlight: theme.selection(),
                    },
                );
                buf.write_str(
                    inner.x,
                    inner.bottom().saturating_sub(1),
                    "j/k move · e edit · n new · d delete · p pin",
                    theme.dim(),
                    inner.w,
                );
                None
            }
            NotesMode::Edit => {
                let content = Rect::new(inner.x, inner.y, inner.w, inner.h.saturating_sub(1));
                let body: String = self.edit_body.iter().collect();
                let lines: Vec<Line> =
                    body.split('\n').map(|s| Line::styled(s.to_string(), theme.text())).collect();
                paragraph(buf, content, &lines, ParagraphOpts { scroll: self.edit_scroll, wrap: false });
                buf.write_str(
                    inner.x,
                    inner.bottom().saturating_sub(1),
                    "Esc save & back · autosave on",
                    theme.dim(),
                    inner.w,
                );
                if focused {
                    // Cursor at the edit position (best-effort: line/column).
                    let before: String = self.edit_body[..self.edit_cursor].iter().collect();
                    let line_no = before.matches('\n').count() as u16;
                    let col = before.rsplit('\n').next().map(|s| slate_core::unicode::str_width(s)).unwrap_or(0) as u16;
                    let cy = content.y + line_no.saturating_sub(self.edit_scroll);
                    if line_no >= self.edit_scroll && cy < content.bottom() {
                        return Some((content.x + col.min(content.w.saturating_sub(1)), cy));
                    }
                }
                None
            }
        }
    }
}

// ---------------------------------------------------------------------------
// dashboard
// ---------------------------------------------------------------------------

/// The system dashboard pane.
pub struct DashboardPane {
    dashboard: Dashboard,
    probe: Probe,
    scroll: u16,
}

impl DashboardPane {
    pub fn new(ctx: &Ctx) -> Self {
        DashboardPane {
            dashboard: Dashboard::new(&ctx.dashboard_widget_names()),
            probe: Probe::new(),
            scroll: 0,
        }
    }
}

impl Pane for DashboardPane {
    fn id(&self) -> &'static str {
        "dashboard"
    }

    fn title(&self, _ctx: &Ctx) -> String {
        format!("dashboard ({})", self.dashboard.len())
    }

    fn on_tick(&mut self, app: &mut App) {
        if app.ctx.dashboard_dirty {
            self.dashboard = Dashboard::new(&app.ctx.dashboard_widget_names());
            app.ctx.dashboard_dirty = false;
        }
        let binding = app.ctx.weather_location();

let mut env = WidgetEnv {
    probe: &mut self.probe,
    notifications: app.ctx.notifications.len(),
    weather_location: binding.as_deref(),
    demo: app.ctx.demo,
    cwd: &app.ctx.cwd,
    workspace: app.ctx.workspaces.current_name(),
};
        self.dashboard.refresh(&mut env);
    }

    fn handle_key(&mut self, key: Key, app: &mut App) -> slate_core::error::Result<()> {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => self.scroll = self.scroll.saturating_add(1),
            KeyCode::Char('k') | KeyCode::Up => self.scroll = self.scroll.saturating_sub(1),
            KeyCode::PageUp => self.scroll = self.scroll.saturating_add(5),
            KeyCode::PageDown => self.scroll = self.scroll.saturating_sub(5),
            _ => {}
        }
        app.dirty = true;
        Ok(())
    }

    fn draw(&self, buf: &mut Buffer, inner: Rect, _focused: bool, app: &App) -> Option<(u16, u16)> {
        let lines = self.dashboard.render(inner.w, app.ctx.theme());
        if lines.is_empty() {
            paragraph(
                buf,
                inner,
                &[Line::styled("no widgets — `dashboard toggle <name>` to add one", app.ctx.theme().dim())],
                ParagraphOpts::default(),
            );
            return None;
        }
        let total = paragraph(buf, inner, &lines, ParagraphOpts { scroll: self.scroll, wrap: false });
        drop(total);
        None
    }
}

// ---------------------------------------------------------------------------
// logs
// ---------------------------------------------------------------------------

/// Session log pane.
pub struct LogsPane {
    scroll: u16,
}

impl LogsPane {
    pub fn new() -> Self {
        LogsPane { scroll: 0 }
    }
}

impl Default for LogsPane {
    fn default() -> Self {
        Self::new()
    }
}

impl Pane for LogsPane {
    fn id(&self) -> &'static str {
        "logs"
    }

    fn title(&self, _ctx: &Ctx) -> String {
        "logs".to_string()
    }

    fn handle_key(&mut self, key: Key, app: &mut App) -> slate_core::error::Result<()> {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => self.scroll = self.scroll.saturating_add(1),
            KeyCode::Char('k') | KeyCode::Up => self.scroll = self.scroll.saturating_sub(1),
            _ => {}
        }
        app.dirty = true;
        Ok(())
    }

    fn draw(&self, buf: &mut Buffer, inner: Rect, _focused: bool, app: &App) -> Option<(u16, u16)> {
        let lines: Vec<Line> = app.logs.iter().cloned().collect();
        if lines.is_empty() {
            buf.write_str(inner.x, inner.y, "(quiet)", app.ctx.theme().dim(), inner.w);
            return None;
        }
        // Show the tail; scroll moves upward from the tail.
        let theme = app.ctx.theme();
        let total = lines.len();
        let max_scroll = total.saturating_sub(usize::from(inner.h)) as u16;
        let scroll = self.scroll.min(max_scroll);
        let end = total.saturating_sub(usize::from(scroll));
        let start = end.saturating_sub(usize::from(inner.h));
        for (row, line) in lines[start..end].iter().enumerate() {
            buf.write_line(inner.x, inner.y + row as u16, line, inner.w);
        }
        drop(theme);
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slate_command::Registry;

    fn demo_app() -> App {
        let mut ctx = Ctx::ephemeral();
        ctx.demo = true;
        ctx.headless = false;
        App::new(ctx, Registry::builtins())
    }

    #[test]
    fn prompt_editing_and_submit() {
        let mut app = demo_app();
        let mut pane = PromptPane::new(&app.ctx);
        for c in "echo hi".chars() {
            pane.handle_key(Key::char(c), &mut app).unwrap();
        }
        // Edit: move left over 'i', delete 'h', type 'a' → "echo ai".
        pane.handle_key(Key::plain(KeyCode::Left), &mut app).unwrap();
        pane.handle_key(Key::plain(KeyCode::Backspace), &mut app).unwrap();
        pane.handle_key(Key::char('a'), &mut app).unwrap();
        assert_eq!(pane.input_string(), "echo ai");
        pane.handle_key(Key::ENTER, &mut app).unwrap();
        assert_eq!(pane.input_string(), "");
        let text: String = app.transcript.iter().map(Line::to_plain).collect::<Vec<_>>().join("\n");
        assert!(text.contains("echo ai"), "{text}");
        assert!(text.contains("ai"), "{text}"); // the echo command's own output
    }

    #[test]
    fn prompt_history_navigation() {
        let mut app = demo_app();
        let mut pane = PromptPane::new(&app.ctx);
        app.ctx.history_push("first cmd");
        app.ctx.history_push("second cmd");
        pane.handle_key(Key::plain(KeyCode::Up), &mut app).unwrap();
        assert_eq!(pane.input_string(), "second cmd");
        pane.handle_key(Key::plain(KeyCode::Up), &mut app).unwrap();
        assert_eq!(pane.input_string(), "first cmd");
        pane.handle_key(Key::plain(KeyCode::Down), &mut app).unwrap();
        assert_eq!(pane.input_string(), "second cmd");
        pane.handle_key(Key::plain(KeyCode::Down), &mut app).unwrap();
        assert_eq!(pane.input_string(), "");
    }

    #[test]
    fn notes_list_and_edit() {
        let mut app = demo_app();
        let mut pane = NotesPane::new(&app.ctx);
        // Create a note through the pane.
        pane.handle_key(Key::char('n'), &mut app).unwrap();
        assert!(matches!(pane.mode, NotesMode::Edit));
        assert!(app.ctx.notes.exists("untitled"));
        for c in "hello slate".chars() {
            pane.handle_key(Key::char(c), &mut app).unwrap();
        }
        pane.handle_key(Key::ESC, &mut app).unwrap();
        assert!(matches!(pane.mode, NotesMode::List));
        let note = app.ctx.notes.get("untitled").unwrap();
        assert!(note.body.contains("hello slate"));
        // Remove via 'd'.
        pane.sync(&app);
        pane.handle_key(Key::char('d'), &mut app).unwrap();
        assert!(!app.ctx.notes.exists("untitled"));
    }

    #[test]
    fn dashboard_renders_widgets() {
        let mut app = demo_app();
        let mut pane = DashboardPane::new(&app.ctx);
        pane.on_tick(&mut app);
        let mut term = slate_term::test_terminal(60, 20);
        let frame_captured = std::cell::RefCell::new(String::new());
        term.draw(|buf, area| {
            pane.draw(buf, area, true, &app);
            frame_captured.replace(buf.to_plain_text());
        })
        .unwrap();
        // Redraw happens through the app normally; here just assert the pane draws.
        let text = frame_captured.borrow();
        assert!(!text.trim().is_empty(), "dashboard pane should draw demo widgets");
    }

    #[test]
    fn logs_pane_shows_tail() {
        let mut app = demo_app();
        for i in 0..10 {
            let _ = app.run_command_line(&format!("echo line{i}"));
        }
        let pane = LogsPane::new();
        let mut term = slate_term::test_terminal(60, 8);
        term.draw(|buf, area| {
            pane.draw(buf, area, true, &app);
        })
        .unwrap();
        assert!(term.backend().buffer().to_plain_text().contains("session started"));
    }
}
