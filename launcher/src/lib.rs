//! `slate-launcher` — universal search.
//!
//! Instead of a launcher window, Slate searches *everything* from the prompt:
//! commands, notes, files, plugins, themes, workspaces, settings, history and
//! bookmarks. This crate provides the index and the matcher; the command
//! layer assembles the item sources.
//!
//! The matcher is a simplified [Skim](https://github.com/lotabout/skim)-style
//! scorer: case-insensitive subsequence match with consecutive-match and
//! word/camel-case boundary bonuses.

#![forbid(unsafe_code)]

use slate_core::style::{Line, Span, Style};

/// What a search result represents.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ItemKind {
    Command,
    Note,
    File,
    Plugin,
    Theme,
    Workspace,
    Setting,
    History,
    Bookmark,
    Document,
}

impl ItemKind {
    pub fn label(self) -> &'static str {
        match self {
            ItemKind::Command => "cmd",
            ItemKind::Note => "note",
            ItemKind::File => "file",
            ItemKind::Plugin => "plugin",
            ItemKind::Theme => "theme",
            ItemKind::Workspace => "ws",
            ItemKind::Setting => "set",
            ItemKind::History => "hist",
            ItemKind::Bookmark => "url",
            ItemKind::Document => "doc",
        }
    }
}

/// One searchable entry.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub kind: ItemKind,
    pub title: String,
    /// Secondary text: command summary, file path, note preview…
    pub detail: String,
    /// The prompt line executed when the result is accepted.
    pub action: String,
    /// Manual ranking boost (built-in commands > history, etc.).
    pub boost: i64,
}

impl Item {
    pub fn new(kind: ItemKind, title: impl Into<String>, detail: impl Into<String>, action: impl Into<String>) -> Self {
        Item { kind, title: title.into(), detail: detail.into(), action: action.into(), boost: 0 }
    }

    pub fn with_boost(mut self, boost: i64) -> Self {
        self.boost = boost;
        self
    }

    /// Render as a result row: `[kind] title — detail`.
    pub fn to_line(&self) -> Line {
        Line::new(vec![
            Span::styled(format!("[{}]", self.kind.label()), Style::new()),
            Span::plain(" "),
            Span::plain(self.title.clone()),
        ])
    }
}

/// The search index.
#[derive(Clone, Debug, Default)]
pub struct Index {
    items: Vec<Item>,
}

impl Index {
    pub fn new(items: Vec<Item>) -> Self {
        Index { items }
    }

    pub fn items(&self) -> &[Item] {
        &self.items
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Fuzzy query. Sorts by score (desc), then title (asc). Empty query
    /// returns items by boost.
    pub fn query(&self, q: &str, limit: usize) -> Vec<&Item> {
        let q = q.trim();
        let mut scored: Vec<(i64, &Item)> = if q.is_empty() {
            self.items.iter().map(|i| (i.boost, i)).collect()
        } else {
            self.items
                .iter()
                .filter_map(|i| {
                    let title = fuzzy_score(&i.title, q)?;
                    let detail = fuzzy_score(&i.detail, q).map(|s| s / 4).unwrap_or(0);
                    Some((title + detail + i.boost, i))
                })
                .collect()
        };
        scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.title.cmp(&b.1.title)));
        scored.truncate(limit);
        scored.into_iter().map(|(_, i)| i).collect()
    }
}

/// Case-insensitive subsequence score with consecutive/boundary bonuses.
/// Returns `None` when `pattern` is not a subsequence of `choice`.
pub fn fuzzy_score(choice: &str, pattern: &str) -> Option<i64> {
    if pattern.is_empty() {
        return Some(0);
    }
    let orig: Vec<char> = choice.chars().collect();
    let cl: Vec<char> = choice.to_lowercase().chars().collect();
    let pl: Vec<char> = pattern.to_lowercase().chars().collect();
    if pl.len() > cl.len() {
        return None;
    }
    let boundary: Vec<bool> = orig
        .iter()
        .enumerate()
        .map(|(i, &c)| {
            if i == 0 {
                return true;
            }
            let p = orig[i - 1];
            matches!(p, ' ' | '-' | '_' | '/' | '.' | ':' | '(')
                || (p.is_lowercase() && c.is_uppercase())
        })
        .collect();

    let first = pl[0];
    let mut best: Option<i64> = None;
    for start in 0..=cl.len() - pl.len() {
        if cl[start] != first {
            continue;
        }
        if let Some(score) = match_from(&cl, &pl, &boundary, start) {
            best = Some(best.map_or(score, |b: i64| b.max(score)));
        }
    }
    best
}

fn match_from(cl: &[char], pl: &[char], boundary: &[bool], start: usize) -> Option<i64> {
    let mut score: i64 = 0;
    let mut idx = start;
    let mut prev: Option<usize> = None;
    for (j, &p) in pl.iter().enumerate() {
        let mut found = None;
        let mut i = idx;
        while i < cl.len() {
            if cl[i] == p {
                found = Some(i);
                break;
            }
            i += 1;
        }
        let i = found?;
        score += 10;
        if j == 0 && i == 0 && cl.len() == pl.len() {
            score += 100; // exact match
        }
        if boundary[i] {
            score += 6;
        }
        if let Some(pv) = prev {
            if i == pv + 1 {
                score += 8; // consecutive
            } else {
                score -= ((i - pv - 1) as i64).min(10); // gap penalty (capped)
            }
        }
        prev = Some(i);
        idx = i + 1;
    }
    Some(score - (start as i64).min(20))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subsequence_required() {
        assert!(fuzzy_score("notes add", "na").is_some());
        assert!(fuzzy_score("volume", "vol").is_some());
        assert!(fuzzy_score("wifi", "wfi").is_some());
        assert!(fuzzy_score("calendar", "xyz").is_none());
        assert!(fuzzy_score("", "a").is_none());
        assert_eq!(fuzzy_score("anything", ""), Some(0));
    }

    #[test]
    fn ordering_prefers_exact_and_boundaries() {
        let exact = fuzzy_score("theme", "theme").unwrap();
        let partial = fuzzy_score("theme set", "theme").unwrap();
        assert!(exact > partial);

        let boundary = fuzzy_score("notes-add", "na").unwrap();
        let middle = fuzzy_score("banana", "na").unwrap();
        assert!(boundary > middle);

        // CamelCase boundaries count.
        assert!(fuzzy_score("NotesAdd", "na") >= fuzzy_score("notesadd", "na").unwrap_or(0));
    }

    #[test]
    fn case_insensitive() {
        assert!(fuzzy_score("README.md", "readme").is_some());
        assert!(fuzzy_score("Setting", "set").is_some());
    }

    #[test]
    fn index_query_sorts() {
        let items = vec![
            Item::new(ItemKind::Command, "theme", "set the theme", "theme").with_boost(10),
            Item::new(ItemKind::Command, "theme set", "change theme", "theme set").with_boost(10),
            Item::new(ItemKind::Note, "theme ideas", "my note", "notes show theme-ideas"),
            Item::new(ItemKind::File, "thematics.pdf", "/home/u/thematics.pdf", "open /home/u/thematics.pdf"),
        ];
        let index = Index::new(items);
        let hits = index.query("theme", 10);
        assert_eq!(hits.len(), 4);
        assert_eq!(hits[0].title, "theme"); // exact + boost first
        let limited = index.query("theme", 2);
        assert_eq!(limited.len(), 2);
        let everything = index.query("", 10);
        assert_eq!(everything[0].boost, 10);
        assert!(index.query("zzz", 10).is_empty());
    }

    #[test]
    fn detail_contributes() {
        let items = vec![
            Item::new(ItemKind::File, "a.pdf", "/docs/quantum-tunneling.pdf", "open a"),
            Item::new(ItemKind::File, "b.pdf", "/docs/recipes.pdf", "open b"),
        ];
        let index = Index::new(items);
        let hits = index.query("quantum", 5);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].title, "a.pdf");
    }
}
