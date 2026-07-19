//! `theme` — list, preview, and switch themes.

use slate_core::effect::Effect;
use slate_core::error::{Error, Result};
use slate_core::style::{Line, Span, Style};

use crate::{Command, Ctx, Output, Registry};

pub fn register(r: &mut Registry) {
    r.register(ThemeCmd);
}

struct ThemeCmd;

impl Command for ThemeCmd {
    fn name(&self) -> &'static str {
        "theme"
    }
    fn about(&self) -> &'static str {
        "list, preview, and switch themes"
    }
    fn usage(&self) -> &'static str {
        "theme [list|set <name>|show|path]"
    }

    fn run(&self, args: &[String], ctx: &mut Ctx) -> Result<Output> {
        let sub = args.first().map(String::as_str).unwrap_or("list");
        match sub {
            "list" => {
                let current = ctx.themes.current_name();
                let mut out = Output::new();
                for name in ctx.themes.names() {
                    let marker = if name == current { "●" } else { " " };
                    let style =
                        if name == current { ctx.theme().accent() } else { ctx.theme().text() };
                    out.line(Line::new(vec![
                        Span::styled(format!("{marker} "), style),
                        Span::styled(name, style),
                    ]));
                }
                Ok(out)
            }
            "set" => {
                let Some(name) = args.get(1) else {
                    return Err(Error::invalid("usage: theme set <name>"));
                };
                ctx.set_theme(name)?;
                let mut out = Output::text(format!("theme: {name}"));
                out.effect(Effect::SetTheme(name.to_string()));
                out.effect(Effect::Redraw);
                Ok(out)
            }
            "show" => {
                let t = ctx.theme();
                let p = t.palette;
                let mut out = Output::new();
                out.line(Line::styled(format!("theme: {}", t.name), t.accent()));
                for (label, color) in [
                    ("bg", p.bg),
                    ("fg", p.fg),
                    ("muted", p.muted),
                    ("accent", p.accent),
                    ("accent2", p.accent2),
                    ("ok", p.ok),
                    ("warn", p.warn),
                    ("error", p.error),
                    ("selection", p.selection),
                    ("border", p.border),
                    ("border_focused", p.border_focused),
                    ("bar_bg", p.bar_bg),
                    ("bar_fg", p.bar_fg),
                ] {
                    let swatch = Style::new().bg(color);
                    out.line(Line::new(vec![
                        Span::styled(format!("{label:<14}"), t.dim()),
                        Span::styled("        ", swatch),
                        Span::styled(format!("  {color}"), t.text()),
                    ]));
                }
                out.line(Line::styled(
                    format!("borders: {:?} · unicode: {}", t.borders, t.unicode),
                    t.dim(),
                ));
                Ok(out)
            }
            "path" => Ok(Output::text(
                slate_config::paths::themes_dir().to_string_lossy().into_owned(),
            )),
            other => Err(Error::invalid(format!(
                "unknown theme subcommand '{other}' (list|set|show|path)"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_commands() {
        let r = Registry::builtins();
        let mut ctx = Ctx::ephemeral();
        ctx.cfg.path =
            std::env::temp_dir().join(format!("slate-themecmd-{}.toml", std::process::id()));

        let out = r.exec("theme", &mut ctx).unwrap();
        let text = out.to_plain_string();
        assert!(text.contains("slate-dark"));
        assert!(text.contains("●"));

        let out = r.exec("theme set contrast", &mut ctx).unwrap();
        assert_eq!(ctx.themes.current_name(), "contrast");
        assert!(out.effects.contains(&Effect::SetTheme("contrast".into())));

        let out = r.exec("theme show", &mut ctx).unwrap();
        assert!(out.to_plain_string().contains("accent"));
        assert!(r.exec("theme set missing", &mut ctx).is_err());
        assert!(r.exec("theme bogus", &mut ctx).is_err());
        let _ = std::fs::remove_file(&ctx.cfg.path);
    }
}
