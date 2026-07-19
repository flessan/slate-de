//! Meta and session commands: help, version, echo, run, open, quit, …

use slate_core::effect::Effect;
use slate_core::error::{Error, Result};
use slate_core::style::{Line, Span};
use slate_core::{proc, VERSION};

use crate::{Command, Ctx, Output, Registry};

pub fn register(r: &mut Registry) {
    r.register(Help);
    r.register(About);
    r.register(Version);
    r.register(Keys);
    r.register(Doctor);
    r.register(Echo);
    r.register(Clear);
    r.register(Quit);
    r.register(Reload);
    r.register(History);
    r.register(Run);
    r.register(Cd);
    r.register(Pwd);
    r.register(Open);
    r.register(Notify);
}

// ---------------------------------------------------------------------------
// help
// ---------------------------------------------------------------------------

struct Help;

impl Command for Help {
    fn name(&self) -> &'static str {
        "help"
    }
    fn aliases(&self) -> &'static [&'static str] {
        &["?"]
    }
    fn about(&self) -> &'static str {
        "show help for all commands or one command"
    }
    fn usage(&self) -> &'static str {
        "help [command]"
    }

    fn run(&self, args: &[String], ctx: &mut Ctx) -> Result<Output> {
        let theme = ctx.theme();
        let header = |s: &str| Line::styled(s.to_string(), theme.accent());
        let registry = Registry::builtins(); // includes everything users can call
        let mut out = Output::new();
        if let Some(name) = args.first() {
            let Some(cmd) = registry.find(name) else {
                return Err(Error::not_found(format!("no such command '{name}'")));
            };
            out.line(header(cmd.name()));
            out.line(Line::plain(format!("  {}", cmd.about())));
            if !cmd.aliases().is_empty() {
                out.line(Line::plain(format!("  aliases: {}", cmd.aliases().join(", "))));
            }
            if !cmd.usage().is_empty() {
                out.line(Line::new(vec![
                    Span::styled("  usage: ", theme.dim()),
                    Span::styled(cmd.usage(), theme.text()),
                ]));
            }
            return Ok(out);
        }
        out.line(header("Slate — the command line is your desktop"));
        out.line(Line::plain("every feature is a command. type `help <name>` for details.".to_string()));
        out.line(Line::default());
        let width = registry.names().iter().map(|n| n.len()).max().unwrap_or(8) + 2;
        for name in registry.names() {
            let cmd = registry.find(name).unwrap();
            out.line(Line::new(vec![
                Span::styled(format!("  {name:<width$}"), theme.accent2()),
                Span::styled(cmd.about(), theme.dim()),
            ]));
        }
        Ok(out)
    }
}

// ---------------------------------------------------------------------------
// about / version / keys
// ---------------------------------------------------------------------------

struct About;

impl Command for About {
    fn name(&self) -> &'static str {
        "about"
    }
    fn about(&self) -> &'static str {
        "what Slate is"
    }

    fn run(&self, _args: &[String], ctx: &mut Ctx) -> Result<Output> {
        let theme = ctx.theme();
        Ok(Output::from_lines(vec![
            Line::styled(format!("slate v{VERSION} — the CLI-first desktop environment"), theme.accent()),
            Line::styled("the command line is not an app. it is your desktop.", theme.text()),
            Line::default(),
            Line::plain("  everything is a command · everything is a pane".to_string()),
            Line::plain("  everything is searchable · everything is scriptable".to_string()),
            Line::styled(format!("  {}", crate::DOCS_URL), theme.accent2()),
        ]))
    }
}

struct Version;

impl Command for Version {
    fn name(&self) -> &'static str {
        "version"
    }
    fn aliases(&self) -> &'static [&'static str] {
        &["ver"]
    }
    fn about(&self) -> &'static str {
        "print version information"
    }

    fn run(&self, _args: &[String], _ctx: &mut Ctx) -> Result<Output> {
        Ok(Output::text(format!(
            "slate {VERSION} (plugin api {}, layout dsl v1)",
            slate_plugin::API_VERSION
        )))
    }
}

struct Keys;

impl Command for Keys {
    fn name(&self) -> &'static str {
        "keys"
    }
    fn about(&self) -> &'static str {
        "list keyboard shortcuts"
    }

    fn run(&self, _args: &[String], ctx: &mut Ctx) -> Result<Output> {
        let theme = ctx.theme();
        let mut out = Output::new();
        out.line(Line::styled("keyboard shortcuts", theme.accent()));
        const W: usize = 16;
        for (key, action) in crate::KEYBINDINGS {
            out.line(Line::new(vec![
                Span::styled(format!("  {key:<W$}"), theme.accent2()),
                Span::styled(*action, theme.text()),
            ]));
        }
        Ok(out)
    }
}

// ---------------------------------------------------------------------------
// doctor
// ---------------------------------------------------------------------------

struct Doctor;

impl Command for Doctor {
    fn name(&self) -> &'static str {
        "doctor"
    }
    fn about(&self) -> &'static str {
        "check your environment (tools, terminal, directories)"
    }

    fn run(&self, _args: &[String], ctx: &mut Ctx) -> Result<Output> {
        let theme = ctx.theme();
        let mut out = Output::new();
        out.line(Line::styled("slate doctor", theme.accent()));

        let mut push = |ok: bool, label: String, note: &str| {
            let (glyph, style) = if ok { (theme.glyph("ok"), theme.ok()) } else { (theme.glyph("warn"), theme.warn()) };
            out.line(Line::new(vec![
                Span::styled(format!(" {glyph} "), style),
                Span::styled(format!("{label:<28}"), theme.text()),
                Span::styled(note.to_string(), theme.dim()),
            ]));
        };

        // Optional system tools: present = more power, absent = graceful.
        for (tool, why) in [
            ("xdg-open", "open files/URLs"),
            ("git", "git widget, `git` cmd"),
            ("wpctl", "volume widget"),
            ("playerctl", "music widget"),
            ("nmcli", "wifi command"),
            ("bluetoothctl", "bluetooth command"),
            ("brightnessctl", "brightness command"),
            ("curl", "weather, vision"),
            ("zbarimg", "vision qr"),
            ("tesseract", "vision ocr/translate"),
            ("trans", "vision translate"),
            ("grim", "vision screenshot (wayland)"),
            ("docker", "docker widget"),
        ] {
            let ok = proc::exists(tool);
            push(ok, format!("tool: {tool}"), if ok { "found" } else { "optional, missing" });
        }

        // Terminal capabilities.
        let term = std::env::var("TERM").unwrap_or_else(|_| "unset".into());
        push(!term.is_empty() && term != "unset", "term".into(), &term);
        let truecolor = matches!(
            std::env::var("COLORTERM").as_deref(),
            Ok("truecolor") | Ok("24bit")
        );
        push(truecolor, "truecolor".into(), if truecolor { "on" } else { "off (colors degrade)" });

        // Directories.
        let cfg_dir = slate_config::paths::config_dir();
        push(cfg_dir.exists(), "config dir".into(), &cfg_dir.to_string_lossy());
        let notes_ok = ctx.notes.dir().exists();
        push(notes_ok, "notes dir".into(), &ctx.notes.dir().to_string_lossy());
        push(ctx.plugins.dir().exists() || slate_config::paths::ensure_dirs().is_ok(),
             "plugins dir".into(), &ctx.plugins.dir().to_string_lossy());

        // Self state.
        out.line(Line::styled("session", theme.accent()));
        out.line(Line::plain(format!(
            "  {} themes · {} plugins · {} workspaces · {} history entries",
            ctx.themes.names().len(),
            ctx.plugins.list().len(),
            ctx.workspaces.names().len(),
            ctx.history.len()
        )));
        Ok(out)
    }
}

// ---------------------------------------------------------------------------
// echo / clear / quit / reload / history
// ---------------------------------------------------------------------------

struct Echo;

impl Command for Echo {
    fn name(&self) -> &'static str {
        "echo"
    }
    fn about(&self) -> &'static str {
        "print text"
    }
    fn usage(&self) -> &'static str {
        "echo <text..>"
    }

    fn run(&self, args: &[String], _ctx: &mut Ctx) -> Result<Output> {
        Ok(Output::text(args.join(" ")))
    }
}

struct Clear;

impl Command for Clear {
    fn name(&self) -> &'static str {
        "clear"
    }
    fn about(&self) -> &'static str {
        "clear the prompt scrollback"
    }

    fn run(&self, _args: &[String], _ctx: &mut Ctx) -> Result<Output> {
        let mut out = Output::new();
        out.effect(Effect::ClearScrollback);
        out.effect(Effect::Redraw);
        Ok(out)
    }
}

struct Quit;

impl Command for Quit {
    fn name(&self) -> &'static str {
        "quit"
    }
    fn aliases(&self) -> &'static [&'static str] {
        &["exit", "logout"]
    }
    fn about(&self) -> &'static str {
        "end the slate session"
    }

    fn run(&self, _args: &[String], ctx: &mut Ctx) -> Result<Output> {
        if ctx.headless {
            return Ok(Output::text("(headless) would quit the session"));
        }
        let mut out = Output::text("leaving slate…");
        out.effect(Effect::Quit);
        Ok(out)
    }
}

struct Reload;

impl Command for Reload {
    fn name(&self) -> &'static str {
        "reload"
    }
    fn about(&self) -> &'static str {
        "reload configuration from disk"
    }

    fn run(&self, _args: &[String], ctx: &mut Ctx) -> Result<Output> {
        ctx.reload()?;
        let mut out = Output::text("configuration reloaded");
        out.effect(Effect::ReloadConfig);
        Ok(out)
    }
}

struct History;

impl Command for History {
    fn name(&self) -> &'static str {
        "history"
    }
    fn about(&self) -> &'static str {
        "show command history"
    }
    fn usage(&self) -> &'static str {
        "history [count]"
    }

    fn run(&self, args: &[String], ctx: &mut Ctx) -> Result<Output> {
        let count = args.first().and_then(|s| s.parse::<usize>().ok()).unwrap_or(50);
        let start = ctx.history.len().saturating_sub(count);
        let mut out = Output::new();
        for (i, line) in ctx.history[start..].iter().enumerate() {
            out.line(Line::plain(format!("{:>4}  {}", start + i + 1, line)));
        }
        if ctx.history.is_empty() {
            out.line(Line::plain("(history is empty)".to_string()));
        }
        Ok(out)
    }
}

// ---------------------------------------------------------------------------
// run / cd / pwd / open / notify
// ---------------------------------------------------------------------------

struct Run;

impl Command for Run {
    fn name(&self) -> &'static str {
        "run"
    }
    fn aliases(&self) -> &'static [&'static str] {
        &["sh", "!"]
    }
    fn about(&self) -> &'static str {
        "execute a shell snippet (sh -c) and capture output"
    }
    fn usage(&self) -> &'static str {
        "run <shell command..>"
    }

    fn run(&self, args: &[String], ctx: &mut Ctx) -> Result<Output> {
        let script = args.join(" ");
        if script.trim().is_empty() {
            return Err(Error::invalid("usage: run <shell command..>"));
        }
        let (ok, stdout, stderr) = proc::run_shell(&script)?;
        let theme = ctx.theme();
        let mut out = Output::new();
        for line in stdout.lines().take(500) {
            out.line(Line::styled(line.to_string(), theme.text()));
        }
        for line in stderr.lines().take(100) {
            out.line(Line::styled(line.to_string(), theme.warn()));
        }
        if !ok {
            out.line(Line::styled("command exited non-zero".to_string(), theme.error()));
        }
        Ok(out)
    }
}

struct Cd;

impl Command for Cd {
    fn name(&self) -> &'static str {
        "cd"
    }
    fn about(&self) -> &'static str {
        "change the prompt working directory"
    }
    fn usage(&self) -> &'static str {
        "cd [dir]"
    }

    fn run(&self, args: &[String], ctx: &mut Ctx) -> Result<Output> {
        let target = match args.first() {
            None => slate_config::paths::home_dir(),
            Some(p) => {
                let expanded = crate::ctx::expand_tilde(p);
                let cand = std::path::PathBuf::from(&expanded);
                if cand.is_absolute() {
                    cand
                } else {
                    ctx.cwd.join(cand)
                }
            }
        };
        if !target.is_dir() {
            return Err(Error::not_found(format!("directory '{}'", target.display())));
        }
        ctx.cwd = target.canonicalize().unwrap_or(target);
        Ok(Output::text(ctx.cwd.to_string_lossy().into_owned()))
    }
}

struct Pwd;

impl Command for Pwd {
    fn name(&self) -> &'static str {
        "pwd"
    }
    fn about(&self) -> &'static str {
        "print the working directory"
    }

    fn run(&self, _args: &[String], ctx: &mut Ctx) -> Result<Output> {
        Ok(Output::text(ctx.cwd.to_string_lossy().into_owned()))
    }
}

struct Open;

impl Command for Open {
    fn name(&self) -> &'static str {
        "open"
    }
    fn aliases(&self) -> &'static [&'static str] {
        &["browser", "xdg-open"]
    }
    fn about(&self) -> &'static str {
        "open a file or URL with the default application"
    }
    fn usage(&self) -> &'static str {
        "open <path|url>"
    }

    fn run(&self, args: &[String], ctx: &mut Ctx) -> Result<Output> {
        let Some(target) = args.first() else {
            return Err(Error::invalid("usage: open <path|url>"));
        };
        let expanded = if target.starts_with("http://") || target.starts_with("https://") {
            target.clone()
        } else {
            let e = crate::ctx::expand_tilde(target);
            let p = std::path::PathBuf::from(&e);
            let abs = if p.is_absolute() { p } else { ctx.cwd.join(p) };
            abs.to_string_lossy().into_owned()
        };
        proc::open_target(&expanded)?;
        Ok(Output::text(format!("opened {expanded}")))
    }
}

struct Notify;

impl Command for Notify {
    fn name(&self) -> &'static str {
        "notify"
    }
    fn about(&self) -> &'static str {
        "push an in-app notification"
    }
    fn usage(&self) -> &'static str {
        "notify <text..>"
    }

    fn run(&self, args: &[String], ctx: &mut Ctx) -> Result<Output> {
        let text = args.join(" ");
        if text.is_empty() {
            return Err(Error::invalid("usage: notify <text..>"));
        }
        let _id = ctx.notify(slate_core::notify::Level::Info, text.clone());
        let mut out = Output::text(format!("notified: {text}"));
        out.effect(Effect::Notify(text));
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Registry;

    fn reg() -> Registry {
        Registry::builtins()
    }

    #[test]
    fn help_lists_commands_and_details() {
        let r = reg();
        let mut ctx = Ctx::ephemeral();
        let out = r.exec("help", &mut ctx).unwrap();
        let text = out.to_plain_string();
        assert!(text.contains("notes"));
        assert!(text.contains("theme"));
        let out = r.exec("help notes", &mut ctx).unwrap();
        assert!(out.to_plain_string().contains("persistent notes"));
        assert!(r.exec("help nope", &mut ctx).is_err());
    }

    #[test]
    fn echo_run_and_cd() {
        let r = reg();
        let mut ctx = Ctx::ephemeral();
        assert_eq!(r.exec("echo a b  c", &mut ctx).unwrap().to_plain_string(), "a b c");
        let out = r.exec("run echo from-shell", &mut ctx).unwrap();
        assert!(out.to_plain_string().contains("from-shell"));

        let tmp = std::env::temp_dir().to_string_lossy().into_owned();
        r.exec(&format!("cd {tmp}"), &mut ctx).unwrap();
        assert_eq!(r.exec("pwd", &mut ctx).unwrap().to_plain_string(), tmp);
        assert!(r.exec("cd /definitely/not/here", &mut ctx).is_err());
        assert!(r.exec("run", &mut ctx).is_err());
    }

    #[test]
    fn version_and_keys_and_doctor() {
        let r = reg();
        let mut ctx = Ctx::ephemeral();
        let out = r.exec("version", &mut ctx).unwrap();
        assert!(out.to_plain_string().contains("slate"));
        assert!(r.exec("keys", &mut ctx).unwrap().to_plain_string().contains("Ctrl+Space")
            || r.exec("keys", &mut ctx).unwrap().to_plain_string().contains("Alt+1"));
        // doctor must never fail, even with missing tools.
        assert!(r.exec("doctor", &mut ctx).is_ok());
    }

    #[test]
    fn quit_reports_headless() {
        let r = reg();
        let mut ctx = Ctx::ephemeral();
        assert!(r.exec("quit", &mut ctx).unwrap().to_plain_string().contains("headless"));
        ctx.headless = false;
        let out = r.exec("quit", &mut ctx).unwrap();
        assert!(out.effects.contains(&Effect::Quit));
    }
}
