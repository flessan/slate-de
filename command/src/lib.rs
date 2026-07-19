//! `slate-command` — every desktop feature, as a command.
//!
//! The command line is Slate's API surface — the panes, the keyboard
//! shortcuts, `slatectl`, plugins, scripts, they all end up executing the
//! same command lines through [`Registry::exec`].
//!
//! * [`lex`] — shell-lite tokenizer (quotes, escapes)
//! * [`Command`] / [`Registry`] — command contract and dispatcher
//! * [`Ctx`] — everything a command can touch (config, notes, plugins, …)
//! * [`Output`] — styled lines + side [`Effect`]s for the shell

#![forbid(unsafe_code)]

pub mod builtins;
pub mod ctx;

pub use ctx::Ctx;

use slate_core::effect::Effect;
use slate_core::error::{Error, Result};
use slate_core::style::Line;

/// Project website shown by `about`.
pub const DOCS_URL: &str = "https://github.com/flessan/slate-de";

/// The canonical keybinding table (rendered by the `keys` command and used
/// as documentation for the shell's input handler).
pub const KEYBINDINGS: &[(&str, &str)] = &[
    ("Ctrl+Space", "open universal search"),
    ("Alt+1 … Alt+9", "focus pane by number"),
    ("Alt+h/←  Alt+l/→", "focus pane left / right"),
    ("Alt+j/↓  Alt+k/↑", "focus pane down / up"),
    ("Alt+Enter", "zoom/unzoom focused pane"),
    ("Alt+w / Alt+W", "next / previous workspace"),
    ("Tab", "cycle focus forward"),
    ("Escape", "close overlay · back to prompt"),
    ("Typing", "goes to the focused pane (prompt by default)"),
    ("Up / Down", "history (in prompt)"),
    ("PgUp / PgDn", "scroll focused pane"),
    ("Ctrl+l", "clear prompt scrollback"),
    ("Ctrl+u", "clear current input"),
    ("Ctrl+c", "cancel current input"),
    ("Ctrl+q", "quit slate"),
];

/// What a command produced.
#[derive(Debug, Default)]
pub struct Output {
    pub lines: Vec<Line>,
    pub effects: Vec<Effect>,
}

impl Output {
    pub fn new() -> Self {
        Self::default()
    }

    /// Convenience: single plain-text line.
    pub fn text(s: impl Into<String>) -> Self {
        Output { lines: vec![Line::plain(s)], effects: Vec::new() }
    }

    pub fn from_lines(lines: Vec<Line>) -> Self {
        Output { lines, effects: Vec::new() }
    }

    pub fn line(&mut self, line: Line) -> &mut Self {
        self.lines.push(line);
        self
    }

    pub fn effect(&mut self, effect: Effect) -> &mut Self {
        self.effects.push(effect);
        self
    }

    /// Plain-text rendering (headless mode, `slatectl`, tests).
    pub fn to_plain_string(&self) -> String {
        self.lines.iter().map(Line::to_plain).collect::<Vec<_>>().join("\n")
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty() && self.effects.is_empty()
    }
}

/// The contract every command implements.
pub trait Command {
    /// Primary name (`help <name>`).
    fn name(&self) -> &'static str;
    /// Alternative spellings.
    fn aliases(&self) -> &'static [&'static str] {
        &[]
    }
    /// One-line description for `help`.
    fn about(&self) -> &'static str;
    /// Usage line (`help <name>` shows this).
    fn usage(&self) -> &'static str {
        ""
    }
    /// Execute with raw string args (already lexed).
    fn run(&self, args: &[String], ctx: &mut Ctx) -> Result<Output>;
}

/// The dispatcher: name/alias → command.
pub struct Registry {
    cmds: Vec<Box<dyn Command>>,
}

impl Default for Registry {
    fn default() -> Self {
        Self::builtins()
    }
}

impl Registry {
    pub fn new() -> Self {
        Registry { cmds: Vec::new() }
    }

    pub fn register(&mut self, cmd: impl Command + 'static) {
        self.cmds.push(Box::new(cmd));
    }

    /// All built-in commands.
    pub fn builtins() -> Self {
        let mut r = Registry::new();
        builtins::register_all(&mut r);
        r
    }

    pub fn find(&self, name: &str) -> Option<&dyn Command> {
        let lower = name.to_lowercase();
        self.cmds
            .iter()
            .find(|c| c.name() == lower || c.aliases().contains(&lower.as_str()))
            .map(|c| &**c as &dyn Command)
    }

    /// Primary names of all commands (sorted).
    pub fn names(&self) -> Vec<&'static str> {
        let mut names: Vec<&'static str> = self.cmds.iter().map(|c| c.name()).collect();
        names.sort_unstable();
        names
    }

    /// Everything searchable: names + aliases.
    pub fn all_names(&self) -> Vec<&'static str> {
        let mut out = Vec::new();
        for c in &self.cmds {
            out.push(c.name());
            out.extend_from_slice(c.aliases());
        }
        out.sort_unstable();
        out
    }

    pub fn len(&self) -> usize {
        self.cmds.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cmds.is_empty()
    }

    /// Fuzzy suggestions for the "unknown command" error path.
    pub fn suggest(&self, name: &str) -> Vec<String> {
        let mut scored: Vec<(i64, &str)> = self
            .all_names()
            .into_iter()
            .filter_map(|n| slate_launcher::fuzzy_score(n, name).map(|s| (s, n)))
            .collect();
        scored.sort_by(|a, b| b.0.cmp(&a.0));
        scored.truncate(3);
        scored.into_iter().map(|(_, n)| n.to_string()).collect()
    }

    /// Execute one command line (lex → dispatch → record history).
    pub fn exec(&self, line: &str, ctx: &mut Ctx) -> Result<Output> {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return Ok(Output::new());
        }
        let parts = lex(trimmed)?;
        if parts.is_empty() {
            return Ok(Output::new());
        }
        ctx.history_push(trimmed);
        let (name, args) = parts.split_first().unwrap_or((&parts[0], &[][..]));
        match self.find(name) {
            Some(cmd) => cmd.run(args, ctx),
            None => {
                let suggestions = self.suggest(name);
                let hint = if suggestions.is_empty() {
                    " Try `help`.".to_string()
                } else {
                    format!(" Did you mean: {}?", suggestions.join(", "))
                };
                Err(Error::not_found(format!("unknown command '{name}'.{hint}")))
            }
        }
    }

    /// Execute several lines; concatenates outputs (used by `slate run` and
    /// tests). Stops at the first error unless `keep_going`.
    pub fn exec_lines(
        &self,
        lines: &[&str],
        ctx: &mut Ctx,
        keep_going: bool,
    ) -> Result<Output> {
        let mut acc = Output::new();
        for line in lines {
            match self.exec(line, ctx) {
                Ok(mut o) => {
                    acc.lines.append(&mut o.lines);
                    acc.effects.append(&mut o.effects);
                }
                Err(e) => {
                    if keep_going {
                        acc.line(Line::styled(format!("error: {e}"), slate_core::style::Style::new()));
                    } else {
                        return Err(e);
                    }
                }
            }
        }
        Ok(acc)
    }
}

/// Tokenize a command line: whitespace-separated, with `'`/`"` quoting and
/// backslash escapes inside double quotes.
pub fn lex(input: &str) -> Result<Vec<String>> {
    #[derive(PartialEq)]
    enum State {
        Bare,
        Double,
        Single,
    }
    let mut tokens = Vec::new();
    let mut cur = String::new();
    let mut state = State::Bare;
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        match state {
            State::Double => match c {
                '"' => state = State::Bare,
                '\\' => match chars.next() {
                    Some('n') => cur.push('\n'),
                    Some('t') => cur.push('\t'),
                    Some(other) if "\"\\$`".contains(other) => cur.push(other),
                    Some(other) => {
                        cur.push('\\');
                        cur.push(other);
                    }
                    None => return Err(Error::invalid("dangling escape")),
                },
                _ => cur.push(c),
            },
            State::Single => {
                if c == '\'' {
                    state = State::Bare;
                } else {
                    cur.push(c);
                }
            }
            State::Bare => match c {
                c if c.is_whitespace() => {
                    if !cur.is_empty() {
                        tokens.push(std::mem::take(&mut cur));
                    }
                }
                '"' => state = State::Double,
                '\'' => state = State::Single,
                '\\' => match chars.next() {
                    Some(other) => cur.push(other),
                    None => return Err(Error::invalid("dangling escape")),
                },
                _ => cur.push(c),
            },
        }
    }
    match state {
        State::Bare => {}
        State::Double => return Err(Error::invalid("unterminated double quote")),
        State::Single => return Err(Error::invalid("unterminated single quote")),
    }
    if !cur.is_empty() {
        tokens.push(cur);
    }
    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;
    use slate_core::effect::Effect;

    struct Echo;
    struct Quitter;

    impl Command for Echo {
        fn name(&self) -> &'static str {
            "echo"
        }
        fn about(&self) -> &'static str {
            "print arguments"
        }
        fn run(&self, args: &[String], ctx: &mut Ctx) -> Result<Output> {
            let _ = ctx;
            Ok(Output::text(args.join(" ")))
        }
    }

    impl Command for Quitter {
        fn name(&self) -> &'static str {
            "bye"
        }
        fn aliases(&self) -> &'static [&'static str] {
            &["q"]
        }
        fn about(&self) -> &'static str {
            "leave"
        }
        fn run(&self, _args: &[String], _ctx: &mut Ctx) -> Result<Output> {
            let mut out = Output::text("byeee");
            out.effect(Effect::Quit);
            Ok(out)
        }
    }

    #[test]
    fn lex_basics() {
        assert_eq!(lex("a b c").unwrap(), vec!["a", "b", "c"]);
        assert_eq!(lex("  spaced   out ").unwrap(), vec!["spaced", "out"]);
        assert_eq!(lex("one \"two three\" 'four five'").unwrap(),
                   vec!["one", "two three", "four five"]);
        assert_eq!(lex("say \"tab\\there\"").unwrap(), vec!["say", "tab\there"]);
        assert_eq!(lex("esc\\ ape").unwrap(), vec!["esc ape"]);
        assert_eq!(lex("").unwrap(), Vec::<String>::new());
        assert!(lex("open \"quote").is_err());
        assert!(lex("open 'quote").is_err());
    }

    fn test_registry() -> Registry {
        let mut r = Registry::new();
        r.register(Echo);
        r.register(Quitter);
        r
    }

    #[test]
    fn registry_dispatch_and_aliases() {
        let r = test_registry();
        let mut ctx = Ctx::ephemeral();
        let out = r.exec("echo hello world", &mut ctx).unwrap();
        assert_eq!(out.to_plain_string(), "hello world");
        let out = r.exec("q", &mut ctx).unwrap();
        assert!(out.effects.contains(&Effect::Quit));
        assert!(r.exec("   ", &mut ctx).unwrap().is_empty());
        assert!(ctx.history.contains(&"echo hello world".to_string()));
    }

    #[test]
    fn unknown_command_suggests() {
        let r = test_registry();
        let mut ctx = Ctx::ephemeral();
        let err = r.exec("ech", &mut ctx).unwrap_err();
        assert!(err.to_string().contains("unknown command 'ech'"));
        assert!(err.to_string().contains("echo"), "{err}");
    }
}
