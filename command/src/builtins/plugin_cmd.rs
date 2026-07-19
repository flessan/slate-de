//! `plugin` — install, run, and manage plugins.

use std::path::PathBuf;

use slate_core::error::{Error, Result};
use slate_core::style::{Line, Span};

use crate::{Command, Ctx, Output, Registry};

pub fn register(r: &mut Registry) {
    r.register(PluginCmd);
}

struct PluginCmd;

impl Command for PluginCmd {
    fn name(&self) -> &'static str {
        "plugin"
    }
    fn aliases(&self) -> &'static [&'static str] {
        &["plugins"]
    }
    fn about(&self) -> &'static str {
        "install, run, and manage plugins (see docs/PLUGINS.md)"
    }
    fn usage(&self) -> &'static str {
        "plugin <list|info|run|install|remove|enable|disable|search|path> [args..]"
    }

    fn run(&self, args: &[String], ctx: &mut Ctx) -> Result<Output> {
        let theme = ctx.theme();
        match args.first().map(String::as_str).unwrap_or("list") {
            "list" => {
                let mut out = Output::new();
                if ctx.plugins.list().is_empty() {
                    out.line(Line::styled(
                        format!("no plugins installed — see {}", ctx.plugins.dir().display()),
                        theme.dim(),
                    ));
                    return Ok(out);
                }
                for p in ctx.plugins.list() {
                    let (mark, style) = if p.enabled { ("●", theme.ok()) } else { ("○", theme.dim()) };
                    out.line(Line::new(vec![
                        Span::styled(format!("{mark} "), style),
                        Span::styled(format!("{:<16}", p.manifest.name), theme.text()),
                        Span::styled(format!("{:<10}", p.manifest.version), theme.dim()),
                        Span::styled(p.manifest.description.clone(), theme.dim()),
                    ]));
                }
                Ok(out)
            }
            "info" => {
                let name = args.get(1).ok_or_else(|| Error::invalid("usage: plugin info <name>"))?;
                let Some(p) = ctx.plugins.find(name) else {
                    return Err(Error::not_found(format!("plugin '{name}'")));
                };
                let m = &p.manifest;
                Ok(Output::from_lines(vec![
                    Line::styled(format!("{} v{}", m.name, m.version), theme.accent()),
                    Line::plain(format!("  {}", m.description)),
                    Line::styled(format!("  author: {}", m.author), theme.dim()),
                    Line::styled(format!("  provides: {}", m.provides.join(", ")), theme.dim()),
                    Line::styled(format!("  exec: {}", p.exec_path().display()), theme.dim()),
                    Line::styled(
                        format!("  enabled: {}", p.enabled),
                        theme.dim(),
                    ),
                ]))
            }
            "run" => {
                let name = args.get(1).ok_or_else(|| Error::invalid("usage: plugin run <name> [args..]"))?;
                let output = ctx.plugins.run(name, &args[2..])?;
                let mut out = Output::new();
                for line in output.lines().take(200) {
                    out.line(Line::styled(line.to_string(), theme.text()));
                }
                if output.is_empty() {
                    out.line(Line::styled("(plugin produced no output)".to_string(), theme.dim()));
                }
                Ok(out)
            }
            "install" => {
                let src = args.get(1).ok_or_else(|| Error::invalid("usage: plugin install <path>"))?;
                let path = PathBuf::from(crate::ctx::expand_tilde(src));
                let name = ctx.plugins.install(&path)?;
                ctx.search_dirty = true;
                Ok(Output::text(format!("installed plugin '{name}'")))
            }
            "remove" | "rm" | "uninstall" => {
                let name = args.get(1).ok_or_else(|| Error::invalid("usage: plugin remove <name>"))?;
                ctx.plugins.remove(name)?;
                ctx.search_dirty = true;
                Ok(Output::text(format!("removed plugin '{name}'")))
            }
            "enable" => {
                let name = args.get(1).ok_or_else(|| Error::invalid("usage: plugin enable <name>"))?;
                ctx.plugins.set_enabled(name, true)?;
                Ok(Output::text(format!("plugin '{name}' enabled")))
            }
            "disable" => {
                let name = args.get(1).ok_or_else(|| Error::invalid("usage: plugin disable <name>"))?;
                ctx.plugins.set_enabled(name, false)?;
                Ok(Output::text(format!("plugin '{name}' disabled")))
            }
            "search" => {
                let q = args[1..].join(" ");
                let hits = ctx.plugins.search(&q);
                let mut out = Output::new();
                if hits.is_empty() {
                    out.line(Line::styled(format!("no plugins matching '{q}'"), theme.dim()));
                }
                for p in hits {
                    out.line(Line::plain(format!("{} — {}", p.manifest.name, p.manifest.description)));
                }
                Ok(out)
            }
            "path" => Ok(Output::text(ctx.plugins.dir().to_string_lossy().into_owned())),
            other => Err(Error::invalid(format!(
                "unknown plugin subcommand '{other}' — see `help plugin`"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_commands_against_empty_registry() {
        let r = Registry::builtins();
        let mut ctx = Ctx::ephemeral();
        let out = r.exec("plugin list", &mut ctx).unwrap();
        assert!(out.to_plain_string().contains("no plugins"));
        assert!(r.exec("plugin info nope", &mut ctx).is_err());
        assert!(r.exec("plugin remove nope", &mut ctx).is_err());
        assert!(r.exec("plugin badverb", &mut ctx).is_err());
        assert!(r.exec("plugin", &mut ctx).is_ok());
    }

    #[test]
    fn plugin_install_and_run_via_commands() {
        let r = Registry::builtins();
        let mut ctx = Ctx::ephemeral();
        // Create a source plugin in a temp dir.
        let root = std::env::temp_dir().join(format!("slate-plugcmd-{}", std::process::id()));
        let src = root.join("demo");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(
            src.join("plugin.toml"),
            "name = \"demo\"\nversion = \"0.1\"\ndescription = \"demo plugin\"\nexec = \"run.sh\"\nprovides = [\"demo\"]\n",
        )
        .unwrap();
        let script = src.join("run.sh");
        std::fs::write(&script, "#!/bin/sh\necho plugin-says: $1\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        }

        r.exec(&format!("plugin install {}", src.display()), &mut ctx).unwrap();
        let out = r.exec("plugin list", &mut ctx).unwrap();
        assert!(out.to_plain_string().contains("demo"));

        let out = r.exec("plugin run demo hi", &mut ctx).unwrap();
        assert!(out.to_plain_string().contains("plugin-says: hi"));

        let out = r.exec("plugin info demo", &mut ctx).unwrap();
        assert!(out.to_plain_string().contains("demo plugin"));

        r.exec("plugin disable demo", &mut ctx).unwrap();
        assert!(r.exec("plugin run demo", &mut ctx).is_err());
        r.exec("plugin enable demo", &mut ctx).unwrap();

        r.exec("plugin remove demo", &mut ctx).unwrap();
        assert!(ctx.plugins.list().is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }
}
