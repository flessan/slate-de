//! `slate-notes` — plain-text, git-friendly notes.
//!
//! A notebook is a directory of Markdown files (`<slug>.md`):
//!
//! * the note's title is its first `# Heading` line (fallback: file name)
//! * `#tags` anywhere in the text are indexed
//! * pinning is a `<slug>.pin` sidecar file
//! * everything autosaves to disk — notes survive crashes and are
//!   searchable/syncable with any external tool

#![forbid(unsafe_code)]

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use slate_core::civil::DateTime;
use slate_core::error::{Error, Result};

/// One note.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Note {
    pub title: String,
    pub slug: String,
    pub body: String,
    pub tags: Vec<String>,
    pub pinned: bool,
    /// Last modification as unix seconds.
    pub mtime: i64,
}

impl Note {
    /// The body without the leading `# Title` line.
    pub fn content(&self) -> &str {
        let body = self.body.trim_start_matches('\u{FEFF}');
        if let Some(rest) = body.strip_prefix("# ") {
            return rest.split_once('\n').map(|(_, r)| r).unwrap_or("");
        }
        body
    }

    /// First non-empty content line (preview).
    pub fn preview(&self) -> &str {
        self.content().lines().find(|l| !l.trim().is_empty()).unwrap_or("")
    }
}

/// A directory-backed notebook.
#[derive(Clone, Debug)]
pub struct Notebook {
    dir: PathBuf,
}

impl Notebook {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Notebook { dir: dir.into() }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn init(&self) -> io::Result<()> {
        fs::create_dir_all(&self.dir)
    }

    fn file_for(&self, slug: &str) -> PathBuf {
        self.dir.join(format!("{slug}.md"))
    }

    fn pin_for(&self, slug: &str) -> PathBuf {
        self.dir.join(format!("{slug}.pin"))
    }

    fn read_note(&self, path: &Path) -> Result<Note> {
        let body = fs::read_to_string(path)?;
        let slug = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("note")
            .to_string();
        let title = body
            .lines()
            .find_map(|l| l.strip_prefix("# ").map(str::trim))
            .filter(|t| !t.is_empty())
            .unwrap_or(&slug)
            .to_string();
        let pinned = self.pin_for(&slug).exists();
        let mtime = path
            .metadata()
            .and_then(|m| m.modified())
            .map(|t| {
                t.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
            })
            .unwrap_or(0);
        let tags = extract_tags(&body);
        Ok(Note { title, slug, body, tags, pinned, mtime })
    }

    /// All notes, pinned first then newest first.
    pub fn list(&self) -> Result<Vec<Note>> {
        self.init()?;
        let mut notes = Vec::new();
        for entry in fs::read_dir(&self.dir)? {
            let path = entry?.path();
            if path.extension().and_then(|e| e.to_str()) == Some("md") {
                notes.push(self.read_note(&path)?);
            }
        }
        notes.sort_by(|a, b| b.pinned.cmp(&a.pinned).then(b.mtime.cmp(&a.mtime)));
        Ok(notes)
    }

    pub fn get(&self, title_or_slug: &str) -> Result<Note> {
        let slug = self.resolve_slug(title_or_slug)?;
        let path = self.file_for(&slug);
        if !path.exists() {
            return Err(Error::not_found(format!("note '{title_or_slug}'")));
        }
        self.read_note(&path)
    }

    /// Accepts a title, case-insensitively, or a slug.
    fn resolve_slug(&self, title_or_slug: &str) -> Result<String> {
        let plain = slugify(title_or_slug);
        if self.file_for(&plain).exists() {
            return Ok(plain);
        }
        for note in self.list()? {
            if note.title.eq_ignore_ascii_case(title_or_slug) {
                return Ok(note.slug);
            }
        }
        Ok(plain) // for create()
    }

    pub fn exists(&self, title_or_slug: &str) -> bool {
        self.file_for(&slugify(title_or_slug)).exists()
    }

    pub fn create(&self, title: &str) -> Result<Note> {
        self.init()?;
        let slug = self.resolve_slug(title)?;
        let path = self.file_for(&slug);
        if !path.exists() {
            fs::write(&path, format!("# {}\n", title.trim()))?;
        }
        self.get(&slug)
    }

    /// Replace the whole body of a note.
    pub fn write(&self, title: &str, body: &str) -> Result<Note> {
        self.init()?;
        let note = self.create(title)?;
        let body = if body.starts_with("# ") {
            body.to_string()
        } else {
            format!("# {}\n\n{}", note.title, body)
        };
        fs::write(self.file_for(&note.slug), body)?;
        self.get(&note.slug)
    }

    /// Append a line (with timestamp) to a note, creating it if needed.
    pub fn append(&self, title: &str, text: &str) -> Result<Note> {
        let note = self.create(title)?;
        let path = self.file_for(&note.slug);
        let mut body = fs::read_to_string(&path).unwrap_or_default();
        if !body.ends_with('\n') && !body.is_empty() {
            body.push('\n');
        }
        body.push_str(&format!("- {} {}\n", DateTime::now_utc().format_time(), text));
        fs::write(&path, body)?;
        self.get(&note.slug)
    }

    pub fn set_pinned(&self, title: &str, pinned: bool) -> Result<Note> {
        let note = self.resolve_existing(title)?;
        let pin = self.pin_for(&note.slug);
        if pinned {
            fs::write(&pin, b"")?;
        } else if pin.exists() {
            fs::remove_file(&pin)?;
        }
        self.get(&note.slug)
    }

    pub fn remove(&self, title: &str) -> Result<()> {
        let note = self.resolve_existing(title)?;
        fs::remove_file(self.file_for(&note.slug))?;
        let pin = self.pin_for(&note.slug);
        if pin.exists() {
            fs::remove_file(pin)?;
        }
        Ok(())
    }

    fn resolve_existing(&self, title: &str) -> Result<Note> {
        let slug = self.resolve_slug(title)?;
        if !self.file_for(&slug).exists() {
            return Err(Error::not_found(format!("note '{title}'")));
        }
        self.get(&slug)
    }

    /// Case-insensitive substring search over title, body, and tags.
    pub fn search(&self, query: &str) -> Result<Vec<Note>> {
        let q = query.to_lowercase();
        Ok(self
            .list()?
            .into_iter()
            .filter(|n| {
                n.title.to_lowercase().contains(&q)
                    || n.body.to_lowercase().contains(&q)
                    || n.tags.iter().any(|t| t.to_lowercase().contains(&q))
            })
            .collect())
    }

    /// All tags with usage counts.
    pub fn tags(&self) -> Result<Vec<(String, usize)>> {
        let mut tags: Vec<(String, usize)> = Vec::new();
        for note in self.list()? {
            for tag in note.tags {
                match tags.iter_mut().find(|(t, _)| *t == tag) {
                    Some((_, n)) => *n += 1,
                    None => tags.push((tag, 1)),
                }
            }
        }
        tags.sort_by(|a, b| b.1.cmp(&a.1));
        Ok(tags)
    }

    /// The quick-capture note path (`inbox`).
    pub fn quick_capture(&self, text: &str) -> Result<Note> {
        self.append("inbox", text)
    }
}

/// Filesystem-safe slug from a title.
pub fn slugify(title: &str) -> String {
    let mut out = String::new();
    let mut last_dash = true; // strip leading dashes too
    for c in title.trim().chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() {
            out.push(c);
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    let out = out.trim_end_matches('-').to_string();
    if out.is_empty() {
        "note".to_string()
    } else {
        out
    }
}

/// `#tag` tokens from note text (`#word`, letters/digits/`-`/`_`, ≤ 32).
pub fn extract_tags(body: &str) -> Vec<String> {
    let mut tags = Vec::new();
    for word in body.split_whitespace() {
        let Some(tag) = word.strip_prefix('#') else { continue };
        let tag: String = tag
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
            .take(32)
            .collect();
        if !tag.is_empty() && !tags.contains(&tag) {
            tags.push(tag);
        }
    }
    tags
}

#[cfg(test)]
mod tests {
    use super::*;

    fn notebook() -> (Notebook, PathBuf) {
        let dir = std::env::temp_dir()
            .join(format!("slate-notes-test-{}-{}", std::process::id(), DateTime::now_utc().secs));
        let _ = fs::remove_dir_all(&dir);
        let nb = Notebook::new(&dir);
        nb.init().unwrap();
        (nb, dir)
    }

    #[test]
    fn slug_rules() {
        assert_eq!(slugify("Hello, World!"), "hello-world");
        assert_eq!(slugify("  spaces  everywhere  "), "spaces-everywhere");
        assert_eq!(slugify("!!!"), "note");
        assert_eq!(slugify("Café Notes"), "caf-notes"); // non-ascii folded out
        assert_eq!(slugify("rust_lang#1"), "rust-lang-1");
    }

    #[test]
    fn tag_extraction() {
        let tags = extract_tags("shopping #groceries and #home-lab, re #groceries #x!");
        assert_eq!(tags, vec!["groceries", "home-lab", "x"]);
    }

    #[test]
    fn create_append_pin_remove() {
        let (nb, dir) = notebook();
        let n = nb.create("Shopping List").unwrap();
        assert_eq!(n.slug, "shopping-list");
        assert!(nb.file_for("shopping-list").exists());

        let n = nb.append("shopping list", "milk #groceries").unwrap();
        assert!(n.body.contains("milk #groceries"));
        assert_eq!(n.tags, vec!["groceries"]);
        assert_eq!(n.title, "Shopping List");

        let n = nb.set_pinned("shopping list", true).unwrap();
        assert!(n.pinned);
        let list = nb.list().unwrap();
        assert_eq!(list[0].slug, "shopping-list"); // pinned sorts first

        let found = nb.search("groceries").unwrap();
        assert_eq!(found.len(), 1);

        let n2 = nb.set_pinned("shopping", false).unwrap();
        assert!(!n2.pinned);
        nb.remove("shopping list").unwrap();
        assert!(nb.list().unwrap().is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_notes_error() {
        let (nb, dir) = notebook();
        assert!(nb.get("does not exist").is_err());
        assert!(nb.remove("nope").is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_replaces_body_and_preview() {
        let (nb, dir) = notebook();
        let n = nb.write("ideas", "first line\nsecond line").unwrap();
        assert_eq!(n.title, "ideas");
        assert_eq!(n.preview(), "first line");
        let n = nb.write("ideas", "# Renamed Title\n\nx").unwrap();
        assert_eq!(n.title, "Renamed Title");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn tags_index() {
        let (nb, dir) = notebook();
        nb.append("a", "one #x").unwrap();
        nb.append("b", "two #x #y").unwrap();
        let tags = nb.tags().unwrap();
        assert_eq!(tags[0], ("x".to_string(), 2));
        let _ = fs::remove_dir_all(&dir);
    }
}
