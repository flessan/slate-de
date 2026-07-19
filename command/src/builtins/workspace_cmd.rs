//! `workspace` — intelligent named desktops.

use slate_core::effect::Effect;
use slate_core::error::{Error, Result};
use slate_core::layout::LayoutSpec;
use slate_core::style::{Line, Span};

use crate::{Command, Ctx, Output, Registry};

pub fn register(r: &mut Registry) {
    r.register(WorkspaceCmd);
}

struct WorkspaceCmd;

impl Command for WorkspaceCmd {
    fn name(&self) -> &'static str {
        "workspace"
    }
    fn aliases(&self) -> &'static [&'static str] {
        &["ws"]
    }
    fn about(&self) -> &'static str {
        "switch and manage workspaces (coding, writing, gaming, devops, media…)"
    }
    fn usage(&self) -> &'static str {
        "workspace [list|switch <name>|next|prev|set-layout <name> <spec..>]"
    }

    fn run(&self, args: &[String], ctx: &mut Ctx) -> Result<Output> {
        let sub = args.first().map(String::as_str).unwrap_or("list");
        match sub {
            "list" => {
                let mut out = Output::new();
                let current = ctx.workspaces.current_name();
                for ws in ctx.workspaces.iter() {
                    let marker = if ws.name == current { "●" } else { " " };
                    let panes = ws.layout.names().join(", ");
                    let accent = ws.name == current;
                    let style = if accent { ctx.theme().accent() } else { ctx.theme().text() };
                    out.line(Line::new(vec![
                        Span::styled(format!("{marker} "), style),
                        Span::styled(format!("{:<10}", ws.name), style),
                        Span::styled(panes, ctx.theme().dim()),
                    ]));
                }
                Ok(out)
            }
            "switch" | "sw" => {
                let Some(name) = args.get(1) else {
                    return Err(Error::invalid("usage: workspace switch <name>"));
                };
                if !ctx.workspaces.switch(name) {
                    return Err(Error::not_found(format!("workspace '{name}'")));
                }
                ctx.search_dirty = true;
                let mut out = Output::text(format!("workspace: {name}"));
                out.effect(Effect::SwitchWorkspace(name.to_string()));
                out.effect(Effect::Redraw);
                Ok(out)
            }
            "next" => {
                let mut out = Output::new();
                out.effect(Effect::NextWorkspace);
                Ok(out)
            }
            "prev" => {
                let mut out = Output::new();
                out.effect(Effect::PrevWorkspace);
                Ok(out)
            }
            "set-layout" => {
                let Some(name) = args.get(1) else {
                    return Err(Error::invalid("usage: workspace set-layout <name> <spec..>"));
                };
                let spec = args[2..].join(" ");
                if spec.is_empty() {
                    return Err(Error::invalid(
                        "usage: workspace set-layout <name> <spec..>  e.g. `workspace set-layout coding \"(prompt:80 | notes:20):78 / logs:22\"`",
                    ));
                }
                let layout = LayoutSpec::parse(&spec)?;
                ctx.workspaces.set_layout(name, layout);
                // Persist to config so it survives restarts.
                let _ = ctx.cfg.set_and_save(&format!("workspace_layouts.{name}"), &spec);
                ctx.layout_dirty = true;
                let mut out = Output::text(format!("layout set for workspace '{name}'"));
                out.effect(Effect::Redraw);
                Ok(out)
            }
            other => Err(Error::invalid(format!(
                "unknown workspace subcommand '{other}' (list|switch|next|prev|set-layout)"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_commands() {
        let r = Registry::builtins();
        let mut ctx = Ctx::ephemeral();
        ctx.cfg.path =
            std::env::temp_dir().join(format!("slate-wscmd-{}.toml", std::process::id()));

        let out = r.exec("ws", &mut ctx).unwrap();
        let text = out.to_plain_string();
        assert!(text.contains("coding"));
        assert!(text.contains('●'));

        let out = r.exec("ws switch gaming", &mut ctx).unwrap();
        assert_eq!(ctx.workspaces.current_name(), "gaming");
        assert!(out.effects.contains(&Effect::SwitchWorkspace("gaming".into())));
        assert!(r.exec("ws switch nope", &mut ctx).is_err());

        let out = r.exec("ws next", &mut ctx).unwrap();
        assert!(out.effects.contains(&Effect::NextWorkspace));

        let out = r.exec("ws set-layout streams \"prompt:70 | dashboard:30\"", &mut ctx).unwrap();
        assert!(text_ne(&out));
        assert!(ctx.workspaces.contains("streams"));
        assert_eq!(ctx.workspaces.get("streams").unwrap().layout.names(), vec!["prompt", "dashboard"]);
        // Persisted into config too.
        assert_eq!(
            ctx.cfg.get_str("workspace_layouts.streams", "?"),
            "prompt:70 | dashboard:30"
        );
        assert!(r.exec("ws set-layout bad \"(unbalanced\"", &mut ctx).is_err());
        let _ = std::fs::remove_file(&ctx.cfg.path);
    }

    fn text_ne(out: &Output) -> bool {
        !out.to_plain_string().is_empty()
    }
}
