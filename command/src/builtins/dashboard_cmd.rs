//! `dashboard` — configure and focus the dashboard pane.

use slate_core::effect::Effect;
use slate_core::error::{Error, Result};
use slate_core::style::{Line, Span};

use crate::{Command, Ctx, Output, Registry};

pub fn register(r: &mut Registry) {
    r.register(DashboardCmd);
}

struct DashboardCmd;

impl Command for DashboardCmd {
    fn name(&self) -> &'static str {
        "dashboard"
    }
    fn aliases(&self) -> &'static [&'static str] {
        &["dash", "widgets"]
    }
    fn about(&self) -> &'static str {
        "dashboard widgets: cpu, memory, battery, git, music, weather…"
    }
    fn usage(&self) -> &'static str {
        "dashboard [focus|widgets|toggle <name>]"
    }

    fn run(&self, args: &[String], ctx: &mut Ctx) -> Result<Output> {
        let theme = ctx.theme();
        match args.first().map(String::as_str) {
            None | Some("focus") => {
                let mut out = Output::text("dashboard focused (configure with `dashboard toggle <widget>`)");
                out.effect(Effect::FocusPane("dashboard".to_string()));
                Ok(out)
            }
            Some("widgets") => {
                let active = ctx.dashboard_widget_names();
                let mut out = Output::new();
                out.line(Line::styled("available widgets (● = active)", theme.accent()));
                for (name, about) in slate_dashboard::all_widgets() {
                    let on = active.iter().any(|a| a == name);
                    let marker = if on { "●" } else { " " };
                    let style = if on { theme.text() } else { theme.dim() };
                    out.line(Line::new(vec![
                        Span::styled(format!("{marker} "), style),
                        Span::styled(format!("{name:<15}"), style),
                        Span::styled(*about, theme.dim()),
                    ]));
                }
                Ok(out)
            }
            Some("toggle") => {
                let Some(name) = args.get(1) else {
                    return Err(Error::invalid("usage: dashboard toggle <widget>"));
                };
                if slate_dashboard::widget_by_name(name).is_none() {
                    let known: Vec<&str> =
                        slate_dashboard::all_widgets().iter().map(|(n, _)| *n).collect();
                    return Err(Error::not_found(format!(
                        "widget '{name}'. Available: {}",
                        known.join(", ")
                    )));
                }
                let mut active = ctx.dashboard_widget_names();
                let enabled = if let Some(pos) = active.iter().position(|a| a == name) {
                    active.remove(pos);
                    false
                } else {
                    active.push(name.to_string());
                    true
                };
                let joined = active.join(", ");
                ctx.cfg.set_and_save("dashboard.widgets", &format!("[{joined}]"))?;
                ctx.dashboard_dirty = true;
                let mut out = Output::text(format!(
                    "widget '{name}' {}",
                    if enabled { "enabled" } else { "disabled" }
                ));
                out.effect(Effect::Redraw);
                Ok(out)
            }
            Some(other) => Err(Error::invalid(format!(
                "unknown dashboard subcommand '{other}' (focus|widgets|toggle)"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dashboard_commands() {
        let r = Registry::builtins();
        let mut ctx = Ctx::ephemeral();
        ctx.cfg.path =
            std::env::temp_dir().join(format!("slate-dashcmd-{}.toml", std::process::id()));

        let out = r.exec("dashboard", &mut ctx).unwrap();
        assert!(out.effects.contains(&Effect::FocusPane("dashboard".into())));

        let out = r.exec("dashboard widgets", &mut ctx).unwrap();
        let text = out.to_plain_string();
        assert!(text.contains("battery"));
        assert!(text.contains('●'));

        let out = r.exec("dashboard toggle docker", &mut ctx).unwrap();
        assert!(out.to_plain_string().contains("enabled"));
        assert!(ctx.dashboard_widget_names().contains(&"docker".to_string()));
        assert!(ctx.dashboard_dirty);

        let out = r.exec("dashboard toggle docker", &mut ctx).unwrap();
        assert!(out.to_plain_string().contains("disabled"));
        assert!(r.exec("dashboard toggle nope", &mut ctx).is_err());
        let _ = std::fs::remove_file(&ctx.cfg.path);
    }
}
