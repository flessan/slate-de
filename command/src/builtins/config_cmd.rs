//! `config` — inspect and mutate configuration from the prompt.

use slate_core::error::{Error, Result};
use slate_core::effect::Effect;
use slate_core::style::{Line, Span};

use crate::{Command, Ctx, Output, Registry};

pub fn register(r: &mut Registry) {
    r.register(ConfigCmd);
}

struct ConfigCmd;

impl Command for ConfigCmd {
    fn name(&self) -> &'static str {
        "config"
    }
    fn aliases(&self) -> &'static [&'static str] {
        &["cfg", "settings"]
    }
    fn about(&self) -> &'static str {
        "read and write configuration"
    }
    fn usage(&self) -> &'static str {
        "config <get|set|unset|reset|path|keys|list> [args..]"
    }

    fn run(&self, args: &[String], ctx: &mut Ctx) -> Result<Output> {
        let sub = args.first().map(String::as_str).unwrap_or("list");
        match sub {
            "get" => {
                let Some(key) = args.get(1) else {
                    return Err(Error::invalid("usage: config get <path>"));
                };
                match ctx.cfg.get(key) {
                    Some(v) => Ok(Output::text(v.display())),
                    None => Err(Error::not_found(format!("config key '{key}'"))),
                }
            }
            "set" => {
                let Some(key) = args.get(1) else {
                    return Err(Error::invalid("usage: config set <path> <value..>"));
                };
                let raw = args[2..].join(" ");
                if raw.is_empty() {
                    return Err(Error::invalid("usage: config set <path> <value..>"));
                };
                ctx.cfg.set_and_save(key, &raw)?;
                let mut out = Output::text(format!("{key} = {}", ctx.cfg.get(key).map_or(raw.clone(), |v| v.display())));
                apply_set_effects(key, &raw, ctx, &mut out);
                Ok(out)
            }
            "unset" => {
                let Some(key) = args.get(1) else {
                    return Err(Error::invalid("usage: config unset <path>"));
                };
                if !ctx.cfg.doc.remove(key) {
                    return Err(Error::not_found(format!("config key '{key}'")));
                }
                ctx.cfg.save()?;
                Ok(Output::text(format!("unset {key}")))
            }
            "reset" => {
                ctx.cfg.reset()?;
                let mut out = Output::text("configuration reset to defaults");
                out.effect(Effect::ReloadConfig);
                Ok(out)
            }
            "path" => Ok(Output::text(ctx.cfg.path.to_string_lossy().into_owned())),
            "keys" => {
                let mut out = Output::new();
                for key in ctx.cfg.doc.keys() {
                    out.line(Line::plain(key));
                }
                Ok(out)
            }
            "list" => {
                let theme = ctx.theme();
                let mut out = Output::new();
                let width = ctx.cfg.doc.keys().iter().map(|k| k.len()).max().unwrap_or(8) + 2;
                for (key, value) in ctx.cfg.doc.flatten() {
                    out.line(Line::new(vec![
                        Span::styled(format!("{key:<width$}"), theme.accent2()),
                        Span::styled(value, theme.text()),
                    ]));
                }
                Ok(out)
            }
            other => Err(Error::invalid(format!(
                "unknown config subcommand '{other}' (get|set|unset|reset|path|keys|list)"
            ))),
        }
    }
}

/// Some settings go live immediately.
fn apply_set_effects(key: &str, raw: &str, ctx: &mut Ctx, out: &mut Output) {
    match key {
        "theme.name" => {
            if ctx.themes.get(raw).is_some() {
                let _ = ctx.set_theme(raw);
                out.effect(Effect::SetTheme(raw.to_string()));
                out.effect(Effect::Redraw);
            } else {
                let _ = ctx.notify(
                    slate_core::notify::Level::Warn,
                    format!("theme '{raw}' not found (keeping current)"),
                );
            }
        }
        "vision.default_provider" => {
            if !ctx.vision.set_default(raw) {
                let _ = ctx.notify(
                    slate_core::notify::Level::Warn,
                    format!("unknown vision provider '{raw}'"),
                );
            }
        }
        k if k.starts_with("dashboard.") => ctx.dashboard_dirty = true,
        k if k.starts_with("workspace") => ctx.layout_dirty = true,
        _ => {}
    }
    ctx.search_dirty = true;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_roundtrip_via_commands() {
        let r = Registry::builtins();
        let mut ctx = Ctx::ephemeral();
        // Redirect writes to an ephemeral file.
        let path = std::env::temp_dir()
            .join(format!("slate-configcmd-{}.toml", std::process::id()));
        ctx.cfg.path = path.clone();

        let out = r.exec("config get theme.name", &mut ctx).unwrap();
        assert_eq!(out.to_plain_string(), "slate-dark");

        r.exec("config set ui.tick_ms 120", &mut ctx).unwrap();
        assert_eq!(ctx.cfg.get_u16("ui.tick_ms", 0), 120);
        assert!(path.exists(), "config must persist to disk");

        assert!(r.exec("config get ui.tick_ms", &mut ctx).is_ok());
        assert!(r.exec("config get does.not.exist", &mut ctx).is_err());
        assert!(r.exec("config badverb", &mut ctx).is_err());

        r.exec("config reset", &mut ctx).unwrap();
        assert_eq!(r.exec("config get ui.tick_ms", &mut ctx).unwrap().to_plain_string(), "250");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn setting_theme_applies_immediately() {
        let r = Registry::builtins();
        let mut ctx = Ctx::ephemeral();
        ctx.cfg.path = std::env::temp_dir().join(format!("slate-cfgtheme-{}.toml", std::process::id()));
        let out = r.exec("config set theme.name slate-light", &mut ctx).unwrap();
        assert_eq!(ctx.themes.current_name(), "slate-light");
        assert!(out.effects.contains(&Effect::SetTheme("slate-light".into())));
        let out = r.exec("theme set slate-dark", &mut ctx).unwrap();
        assert!(out.effects.contains(&Effect::SetTheme("slate-dark".into())));
        let _ = std::fs::remove_file(&ctx.cfg.path);
        assert!(r.exec("config set theme.name not-a-theme", &mut ctx).is_ok()); // warns, keeps
    }
}
