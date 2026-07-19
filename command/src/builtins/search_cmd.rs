//! `search` — the universal search, from the prompt.

use slate_core::effect::Effect;
use slate_core::error::Result;
use slate_core::style::{Line, Span};
use slate_core::unicode::ellipsize;

use crate::{Command, Ctx, Output, Registry};

pub fn register(r: &mut Registry) {
    r.register(SearchCmd);
}

struct SearchCmd;

impl Command for SearchCmd {
    fn name(&self) -> &'static str {
        "search"
    }
    fn aliases(&self) -> &'static [&'static str] {
        &["find", "s", "/"]
    }
    fn about(&self) -> &'static str {
        "search everything: commands, notes, files, plugins, settings…"
    }
    fn usage(&self) -> &'static str {
        "search [query..]  (bare opens the live overlay)"
    }

    fn run(&self, args: &[String], ctx: &mut Ctx) -> Result<Output> {
        let query = args.join(" ");
        if query.trim().is_empty() {
            let mut out = Output::text("opening search…");
            out.effect(Effect::OpenSearch(String::new()));
            return Ok(out);
        }
        // Keep the index fresh when data changed (cheap when not).
        if ctx.search_dirty || ctx.index.is_empty() {
            let registry = Registry::builtins();
            ctx.refresh_search(&registry);
        }
        let theme = ctx.theme();
        let hits = ctx.index.query(&query, 15);
        let mut out = Output::new();
        if hits.is_empty() {
            out.line(Line::styled(format!("nothing found for '{query}'"), theme.dim()));
            return Ok(out);
        }
        for item in hits {
            let detail = ellipsize(&item.detail, 44);
            out.line(Line::new(vec![
                Span::styled(format!("[{}] ", item.kind.label()), theme.accent2()),
                Span::styled(format!("{:<22.22}", item.title), theme.text()),
                Span::styled(detail, theme.dim()),
            ]));
        }
        if !ctx.headless {
            out.effect(Effect::OpenSearch(query));
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_finds_commands_and_notes() {
        let r = Registry::builtins();
        let mut ctx = Ctx::ephemeral();
        r.exec("notes new quantum notes", &mut ctx).unwrap();
        let out = r.exec("search quantum", &mut ctx).unwrap();
        let text = out.to_plain_string();
        assert!(text.contains("quantum notes"), "{text}");
        assert!(text.contains("[note]"), "{text}");

        let out = r.exec("search theme", &mut ctx).unwrap();
        assert!(out.to_plain_string().contains("theme"));

        let out = r.exec("search zzz-no-such-thing", &mut ctx).unwrap();
        assert!(out.to_plain_string().contains("nothing found"));
    }
}
