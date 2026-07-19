//! `notes` — the persistent notebook, always one word away.

use slate_core::effect::Effect;
use slate_core::error::{Error, Result};
use slate_core::style::{Line, Span};

use crate::{Command, Ctx, Output, Registry};

pub fn register(r: &mut Registry) {
    r.register(NotesCmd);
}

struct NotesCmd;

impl Command for NotesCmd {
    fn name(&self) -> &'static str {
        "notes"
    }
    fn aliases(&self) -> &'static [&'static str] {
        &["note", "nb"]
    }
    fn about(&self) -> &'static str {
        "persistent notes (markdown, tags, pins, search, autosave)"
    }
    fn usage(&self) -> &'static str {
        "notes [new <title>|add <text>|list|show <t>|append <t> <text>|search <q>|pin <t>|unpin <t>|rm <t>|tags|path]"
    }

    fn run(&self, args: &[String], ctx: &mut Ctx) -> Result<Output> {
        let theme = ctx.theme();
        match args.first().map(String::as_str) {
            None => {
                // Bare `notes`: list recent + focus the pane.
                let mut out = list_output(ctx)?;
                out.effect(Effect::FocusPane("notes".to_string()));
                Ok(out)
            }
            Some("new") => {
                let title = args[1..].join(" ");
                if title.is_empty() {
                    return Err(Error::invalid("usage: notes new <title>"));
                }
                let note = ctx.notes.create(&title)?;
                ctx.search_dirty = true;
                let mut out = Output::text(format!("created note '{}'", note.title));
                out.effect(Effect::FocusPane("notes".to_string()));
                Ok(out)
            }
            Some("add") => {
                let text = args[1..].join(" ");
                if text.is_empty() {
                    return Err(Error::invalid("usage: notes add <text>"));
                }
                let note = ctx.notes.quick_capture(&text)?;
                ctx.search_dirty = true;
                Ok(Output::text(format!("captured in '{}'", note.title)))
            }
            Some("append") => {
                let Some(title) = args.get(1) else {
                    return Err(Error::invalid("usage: notes append <title> <text..>"));
                };
                let text = args[2..].join(" ");
                if text.is_empty() {
                    return Err(Error::invalid("usage: notes append <title> <text..>"));
                }
                ctx.notes.append(title, &text)?;
                ctx.search_dirty = true;
                Ok(Output::text(format!("appended to '{title}'")))
            }
            Some("list") => list_output(ctx),
            Some("show") => {
                let title = args[1..].join(" ");
                if title.is_empty() {
                    return Err(Error::invalid("usage: notes show <title>"));
                }
                let note = ctx.notes.get(&title)?;
                let mut out = Output::new();
                out.line(Line::styled(format!("# {}", note.title), theme.accent()));
                for line in note.content().lines() {
                    out.line(Line::styled(line.to_string(), theme.text()));
                }
                if !note.tags.is_empty() {
                    out.line(Line::styled(format!("tags: {}", note.tags.join(" ")), theme.dim()));
                }
                Ok(out)
            }
            Some("search") => {
                let q = args[1..].join(" ");
                if q.is_empty() {
                    return Err(Error::invalid("usage: notes search <query>"));
                }
                let hits = ctx.notes.search(&q)?;
                let mut out = Output::new();
                if hits.is_empty() {
                    out.line(Line::styled(format!("no notes matching '{q}'"), theme.dim()));
                }
                for n in hits {
                    out.line(note_row(ctx, &n));
                }
                Ok(out)
            }
            Some("pin") | Some("unpin") => {
                let pinned = args[0] == "pin";
                let title = args[1..].join(" ");
                if title.is_empty() {
                    return Err(Error::invalid(format!("usage: notes {sub} <title>", sub = if pinned { "pin" } else { "unpin" })));
                }
                ctx.notes.set_pinned(&title, pinned)?;
                Ok(Output::text(format!("{} '{title}'", if pinned { "pinned" } else { "unpinned" })))
            }
            Some("rm") | Some("remove") | Some("delete") => {
                let title = args[1..].join(" ");
                if title.is_empty() {
                    return Err(Error::invalid("usage: notes rm <title>"));
                }
                ctx.notes.remove(&title)?;
                ctx.search_dirty = true;
                Ok(Output::text(format!("deleted '{title}'")))
            }
            Some("tags") => {
                let tags = ctx.notes.tags()?;
                let mut out = Output::new();
                if tags.is_empty() {
                    out.line(Line::styled("no tags yet (use #tag in note text)".to_string(), theme.dim()));
                }
                for (tag, count) in tags {
                    out.line(Line::new(vec![
                        Span::styled(format!("#{tag}"), theme.accent2()),
                        Span::styled(format!("  {count}"), theme.dim()),
                    ]));
                }
                Ok(out)
            }
            Some("path") => Ok(Output::text(ctx.notes.dir().to_string_lossy().into_owned())),
            Some("edit") => Ok(Output::text(
                "focus the notes pane (Alt+<n> or `notes`) and press `e` to edit the selected note".to_string(),
            )),
            Some(other) => Err(Error::invalid(format!(
                "unknown notes subcommand '{other}' — see `help notes`"
            ))),
        }
    }
}

fn note_row(ctx: &Ctx, n: &slate_notes::Note) -> Line {
    let theme = ctx.theme();
    let pin = if n.pinned { theme.glyph("pin") } else { " " };
    Line::new(vec![
        Span::styled(format!("{pin} "), theme.accent2()),
        Span::styled(format!("{:<24}", slate_core::unicode::ellipsize(&n.title, 24)), theme.text()),
        Span::styled(
            format!(" {:<40}", slate_core::unicode::ellipsize(n.preview(), 40)),
            theme.dim(),
        ),
    ])
}

fn list_output(ctx: &mut Ctx) -> Result<Output> {
    let notes = ctx.notes.list()?;
    let mut out = Output::new();
    if notes.is_empty() {
        out.line(Line::styled(
            "no notes yet — try `notes new ideas` or `notes add something`".to_string(),
            ctx.theme().dim(),
        ));
        return Ok(out);
    }
    for n in notes.iter().take(20) {
        out.line(note_row(ctx, n));
    }
    if notes.len() > 20 {
        out.line(Line::styled(format!("… and {} more", notes.len() - 20), ctx.theme().dim()));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_flow() {
        let r = Registry::builtins();
        let mut ctx = Ctx::ephemeral();

        let out = r.exec("notes new Project Ideas", &mut ctx).unwrap();
        assert!(out.effects.contains(&Effect::FocusPane("notes".into())));

        r.exec("notes add remember the milk #personal", &mut ctx).unwrap();
        let out = r.exec("notes list", &mut ctx).unwrap();
        let text = out.to_plain_string();
        assert!(text.contains("inbox"), "{text}");
        assert!(text.contains("Project Ideas"), "{text}");

        let out = r.exec("notes show project ideas", &mut ctx).unwrap();
        assert!(out.to_plain_string().contains("Project Ideas"));

        r.exec("notes pin project ideas", &mut ctx).unwrap();
        assert!(ctx.notes.get("project ideas").unwrap().pinned);

        let out = r.exec("notes search milk", &mut ctx).unwrap();
        assert!(out.to_plain_string().contains("inbox"));

        let out = r.exec("notes tags", &mut ctx).unwrap();
        assert!(out.to_plain_string().contains("#personal"));

        r.exec("notes rm project ideas", &mut ctx).unwrap();
        assert!(r.exec("notes show project ideas", &mut ctx).is_err());
        assert!(r.exec("notes nonsense", &mut ctx).is_err());
    }

    #[test]
    fn quick_capture_timestamped() {
        let r = Registry::builtins();
        let mut ctx = Ctx::ephemeral();
        r.exec("notes add first entry", &mut ctx).unwrap();
        let inbox = ctx.notes.get("inbox").unwrap();
        assert!(inbox.body.contains("first entry"));
        assert!(inbox.body.contains('-')); // "- HH:MM text"
    }
}
