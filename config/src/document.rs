//! Ordered document model with dotted-path access and TOML serialization.

use slate_core::error::{Error, Result};

use crate::value::{escape_basic, table_get, table_get_mut, Value};

/// An ordered TOML document.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Document {
    pub root: Value,
}

impl Document {
    pub fn new() -> Self {
        Document { root: Value::Table(Vec::new()) }
    }

    pub fn from_value(root: Value) -> Self {
        Document { root }
    }

    /// Borrow the value at dotted `path` (`"ui.tick_ms"`).
    pub fn get(&self, path: &str) -> Option<&Value> {
        let mut node = &self.root;
        for seg in path.split('.') {
            let Value::Table(table) = node else { return None };
            node = table_get(table, seg)?;
        }
        Some(node)
    }

    pub fn contains(&self, path: &str) -> bool {
        self.get(path).is_some()
    }

    /// Set `path`, creating intermediate tables. Refuses to overwrite a
    /// table with a scalar (destructive; use [`Document::remove`] first).
    pub fn set(&mut self, path: &str, value: Value) -> Result<()> {
        let segments: Vec<&str> = path.split('.').collect();
        if segments.iter().any(|s| s.is_empty()) {
            return Err(Error::invalid(format!("bad key path '{path}'")));
        }
        set_recursive(&mut self.root, &segments, value, path)
    }

    /// Remove `path`; returns whether anything was removed.
    pub fn remove(&mut self, path: &str) -> bool {
        let segments: Vec<&str> = path.split('.').collect();
        remove_recursive(&mut self.root, &segments)
    }

    /// Deep-merge another document into this one (other wins on conflicts,
    /// tables merge recursively).
    pub fn merged(mut self, overlay: Document) -> Document {
        deep_merge(&mut self.root, overlay.root);
        self
    }

    /// All leaf entries as `(path, display-value)` pairs (document order).
    pub fn flatten(&self) -> Vec<(String, String)> {
        let mut out = Vec::new();
        if let Value::Table(t) = &self.root {
            flatten_into(String::new(), t, &mut out);
        }
        out
    }

    /// All leaf paths (`config keys`).
    pub fn keys(&self) -> Vec<String> {
        self.flatten().into_iter().map(|(k, _)| k).collect()
    }

    /// Serialize back to TOML (stable, human-readable).
    pub fn to_toml_string(&self) -> String {
        let mut out = String::from("# Slate configuration\n# Docs: docs/CONFIG.md\n");
        if let Value::Table(t) = &self.root {
            write_table(&mut out, "", t);
        }
        out
    }
}

fn set_recursive(
    node: &mut Value,
    path: &[&str],
    value: Value,
    full_path: &str,
) -> Result<()> {
    let Some((seg, rest)) = path.split_first() else { return Ok(()) };
    let Value::Table(table) = node else {
        return Err(Error::invalid(format!("'{seg}' is a value, not a table")));
    };
    if rest.is_empty() {
        if let Some(existing) = table_get_mut(table, seg) {
            if matches!(existing, Value::Table(_)) && !matches!(value, Value::Table(_)) {
                return Err(Error::invalid(format!("'{full_path}' is a table; unset it first")));
            }
            *existing = value;
        } else {
            table.push(((*seg).to_string(), value));
        }
        return Ok(());
    }
    if table_get_mut(table, seg).is_none() {
        table.push(((*seg).to_string(), Value::Table(Vec::new())));
    }
    let next = table_get_mut(table, seg)
        .ok_or_else(|| Error::invalid(format!("cannot descend into '{seg}'")))?;
    if !matches!(next, Value::Table(_)) {
        return Err(Error::invalid(format!("'{seg}' is a value, not a table")));
    }
    set_recursive(next, rest, value, full_path)
}

fn remove_recursive(node: &mut Value, path: &[&str]) -> bool {
    let Some((seg, rest)) = path.split_first() else { return false };
    let Value::Table(table) = node else { return false };
    if rest.is_empty() {
        if let Some(pos) = table.iter().position(|(k, _)| k == seg) {
            table.remove(pos);
            return true;
        }
        return false;
    }
    match table_get_mut(table, seg) {
        Some(next) => remove_recursive(next, rest),
        None => false,
    }
}

fn deep_merge(base: &mut Value, overlay: Value) {
    match (base, overlay) {
        (Value::Table(b), Value::Table(o)) => {
            for (k, v) in o {
                match table_get_mut(b, k.as_str()) {
                    Some(slot) => deep_merge(slot, v),
                    None => b.push((k, v)),
                }
            }
        }
        (slot, v) => *slot = v,
    }
}

fn flatten_into(prefix: String, table: &[(String, Value)], out: &mut Vec<(String, String)>) {
    for (k, v) in table {
        let path = if prefix.is_empty() { k.clone() } else { format!("{prefix}.{k}") };
        match v {
            Value::Table(t) => flatten_into(path, t, out),
            _ => out.push((path, v.display())),
        }
    }
}

fn key_toml(k: &str) -> String {
    let bare = k
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if bare {
        k.to_string()
    } else {
        format!("\"{}\"", escape_basic(k))
    }
}

fn write_table(out: &mut String, prefix: &str, table: &[(String, Value)]) {
    // Scalars and arrays first, sub-tables after (TOML structural rule).
    for (k, v) in table {
        if !matches!(v, Value::Table(_)) {
            out.push_str(&format!("{} = {}\n", key_toml(k), v.to_toml_value()));
        }
    }
    for (k, v) in table {
        if let Value::Table(t) = v {
            let path = if prefix.is_empty() { key_toml(k) } else { format!("{prefix}.{}", key_toml(k)) };
            out.push_str(&format!("\n[{path}]\n"));
            write_table(out, &path, t);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser;

    fn doc(src: &str) -> Document {
        Document::from_value(parser::parse(src).unwrap())
    }

    #[test]
    fn get_set_remove_paths() {
        let mut d = doc("[ui]\ntick = 250");
        assert_eq!(d.get("ui.tick"), Some(&Value::Int(250)));
        d.set("ui.tick", Value::Int(100)).unwrap();
        d.set("brand.new.key", Value::Bool(true)).unwrap();
        assert_eq!(d.get("brand.new.key"), Some(&Value::Bool(true)));
        assert!(d.set("ui", Value::Int(1)).is_err()); // table overwrite refused
        assert!(d.remove("ui.tick"));
        assert!(!d.contains("ui.tick"));
        assert!(!d.remove("ui.tick"));
        assert!(d.set("ui.tick.deeper", Value::Int(1)).is_err());
    }

    #[test]
    fn merge_overlay_wins() {
        let base = doc("[theme]\nname = \"a\"\n[ui]\ntick = 250\ngap = 0");
        let over = doc("[theme]\nname = \"b\"\n[ui]\ntick = 100");
        let merged = base.merged(over);
        assert_eq!(merged.get("theme.name"), Some(&Value::Str("b".into())));
        assert_eq!(merged.get("ui.tick"), Some(&Value::Int(100)));
        assert_eq!(merged.get("ui.gap"), Some(&Value::Int(0))); // kept from base
    }

    #[test]
    fn flatten_lists_leaves() {
        let d = doc("[a]\nb = 1\n[a.c]\nd = \"x\"\nar = [1, 2]");
        let flat = d.flatten();
        assert!(flat.contains(&("a.b".into(), "1".into())));
        assert!(flat.contains(&("a.c.d".into(), "x".into())));
        assert!(flat.contains(&("a.c.ar".into(), "[1, 2]".into())));
        assert_eq!(d.keys().len(), 3);
    }

    #[test]
    fn roundtrip_serializes() {
        let mut d = doc("[theme]\nname = \"slate-dark\"\n[ui]\ntick = 250\nwidgets = [\"cpu\"]");
        let text = d.to_toml_string();
        let reparsed = parser::parse(&text).unwrap();
        assert_eq!(Document::from_value(reparsed).get("theme.name"),
                   Some(&Value::Str("slate-dark".into())));
        d.set("x.y-z", Value::Str("v".into())).unwrap();
        let text2 = d.to_toml_string();
        assert!(text2.contains("[x]"));
        assert!(text2.contains("y-z = \"v\""));
    }
}
