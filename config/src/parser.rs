//! Dependency-free parser for a pragmatic TOML subset.
//!
//! Supported: `[tables]` (dotted), `key = value`, basic (`"…"`) and literal
//! (`'…'`) strings, integers (dec/hex, `_` separators), floats, booleans, and
//! single-line arrays. Not supported (by design): arrays-of-tables `[[…]]`,
//! inline tables, multi-line strings — these produce clear errors.
//!
//! The subset is strict for files (typos get line-numbered errors) and
//! lenient for CLI input ([`parse_fragment`] treats bare words as strings).

use slate_core::error::{Error, Result};

use crate::value::{table_get_mut, Value};

const WHAT: &str = "toml";

/// Parse a full document into a root table.
pub fn parse(src: &str) -> Result<Value> {
    let mut root = Value::Table(Vec::new());
    let mut current: Vec<String> = Vec::new();
    for (idx, raw) in src.lines().enumerate() {
        let lineno = idx + 1;
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with("[[") {
            return Err(Error::parse_at(WHAT, lineno, "arrays of tables [[…]] are not supported"));
        }
        if let Some(inner) = line.strip_prefix('[') {
            let Some(inner) = inner.strip_suffix(']') else {
                return Err(Error::parse_at(WHAT, lineno, "unterminated table header"));
            };
            let inner = inner.trim();
            if inner.is_empty() {
                return Err(Error::parse_at(WHAT, lineno, "empty table name"));
            }
            current = split_path(inner, lineno)?;
            ensure_table(&mut root, &current)
                .map_err(|e| Error::parse_at(WHAT, lineno, e.to_string()))?;
            continue;
        }
        let eq = find_top_level_char(line, '=').ok_or_else(|| {
            Error::parse_at(WHAT, lineno, "expected `key = value` or `[table]`")
        })?;
        let key_src = line[..eq].trim();
        let value_src = line[eq + 1..].trim();
        if value_src.is_empty() {
            return Err(Error::parse_at(WHAT, lineno, "missing value after '='"));
        }
        let key_path = split_path(key_src, lineno)?;
        let value = parse_value(value_src, lineno)?;
        let mut full = current.clone();
        full.extend(key_path);
        set_at(&mut root, &full, value).map_err(|e| Error::parse_at(WHAT, lineno, e.to_string()))?;
    }
    Ok(root)
}

/// Lenient fragment parser for `config set key <value>`: booleans, numbers,
/// arrays parse as such; everything else becomes a string.
pub fn parse_fragment(raw: &str) -> Value {
    let s = raw.trim();
    if s.starts_with('[') && s.ends_with(']') && s.len() >= 2 {
        let inner = s[1..s.len() - 1].trim();
        if inner.is_empty() {
            return Value::Array(Vec::new());
        }
        let parts = split_top_level_commas(inner);
        return Value::Array(parts.iter().map(|p| parse_fragment(p)).collect());
    }
    match parse_value(s, 0) {
        Ok(v) => v,
        Err(_) => Value::Str(s.to_string()),
    }
}

/// Remove a trailing `#` comment, respecting string quoting.
fn strip_comment(line: &str) -> &str {
    let bytes = line.as_bytes();
    let mut in_basic = false;
    let mut in_literal = false;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'"' if !in_literal => in_basic = !in_basic,
            b'\'' if !in_basic => in_literal = !in_literal,
            b'\\' if in_basic => i += 1, // skip escaped char
            b'#' if !in_basic && !in_literal => return &line[..i],
            _ => {}
        }
        i += 1;
    }
    line
}

/// Find the first occurrence of `c` outside strings.
fn find_top_level_char(line: &str, c: char) -> Option<usize> {
    let bytes = line.as_bytes();
    let needle = c as u8;
    let mut in_basic = false;
    let mut in_literal = false;
    let mut depth = 0usize;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'"' if !in_literal => in_basic = !in_basic,
            b'\'' if !in_basic => in_literal = !in_literal,
            b'[' if !in_basic && !in_literal => depth += 1,
            b']' if !in_basic && !in_literal => depth = depth.saturating_sub(1),
            b if b == needle && !in_basic && !in_literal && depth == 0 => return Some(i),
            b'\\' if in_basic => i += 1,
            _ => {}
        }
        i += 1;
    }
    None
}

fn split_top_level_commas(inner: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut in_basic = false;
    let mut in_literal = false;
    let mut start = 0usize;
    for (i, ch) in inner.char_indices() {
        match ch {
            '"' if !in_literal => in_basic = !in_basic,
            '\'' if !in_basic => in_literal = !in_literal,
            '[' if !in_basic && !in_literal => depth += 1,
            ']' if !in_basic && !in_literal => depth = depth.saturating_sub(1),
            ',' if !in_basic && !in_literal && depth == 0 => {
                out.push(inner[start..i].trim().to_string());
                start = i + ch.len_utf8();
            }
            _ => {}
        }
    }
    let tail = inner[start..].trim();
    if !tail.is_empty() {
        out.push(tail.to_string());
    }
    out
}

/// Split `a.b."c.d"` into segments, honoring quoted parts.
fn split_path(src: &str, lineno: usize) -> Result<Vec<String>> {
    let mut parts = Vec::new();
    let mut cur = String::new();
    let mut in_basic = false;
    let mut in_literal = false;
    let mut chars = src.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if !in_literal => in_basic = !in_basic,
            '\'' if !in_basic => in_literal = !in_literal,
            '.' if !in_basic && !in_literal => {
                push_segment(&mut parts, &mut cur, lineno)?;
            }
            _ => cur.push(c),
        }
    }
    if in_basic || in_literal {
        return Err(Error::parse_at(WHAT, lineno, "unterminated quoted key"));
    }
    push_segment(&mut parts, &mut cur, lineno)?;
    Ok(parts)
}

fn push_segment(parts: &mut Vec<String>, cur: &mut String, lineno: usize) -> Result<()> {
    let seg = cur.trim();
    if seg.is_empty() {
        return Err(Error::parse_at(WHAT, lineno, "empty key segment"));
    }
    parts.push(seg.to_string());
    cur.clear();
    Ok(())
}

/// Ensure tables exist along `path` and return the final one.
/// (Written recursively: iterating while re-borrowing `node` through a child
/// table does not pass the borrow checker.)
fn ensure_table<'a>(root: &'a mut Value, path: &[String]) -> Result<&'a mut Value> {
    let Some((seg, rest)) = path.split_first() else { return Ok(root) };
    let Value::Table(table) = root else {
        return Err(Error::parse(WHAT, format!("key '{seg}' conflicts with a value")));
    };
    if table_get_mut(table, seg).is_none() {
        table.push((seg.clone(), Value::Table(Vec::new())));
    }
    let next = table_get_mut(table, seg)
        .ok_or_else(|| Error::parse(WHAT, "internal: table vanished"))?;
    ensure_table(next, rest)
}

fn set_at(root: &mut Value, path: &[String], value: Value) -> Result<()> {
    let (last, parents) = path
        .split_last()
        .ok_or_else(|| Error::parse(WHAT, "empty key".to_string()))?;
    let table = ensure_table(root, parents)?;
    let Value::Table(entries) = table else {
        return Err(Error::parse(WHAT, "internal error".to_string()));
    };
    if entries.iter().any(|(k, _)| k == last) {
        return Err(Error::parse(WHAT, format!("duplicate key '{last}'")));
    }
    entries.push((last.clone(), value));
    Ok(())
}

/// Parse one value (strict).
fn parse_value(src: &str, lineno: usize) -> Result<Value> {
    let s = src.trim();
    let Some(first) = s.chars().next() else {
        return Err(Error::parse_at(WHAT, lineno, "missing value"));
    };
    match first {
        '"' => {
            let (text, rest) = parse_quoted(&s[1..], '"', true, lineno)?;
            if !rest.trim().is_empty() {
                return Err(Error::parse_at(WHAT, lineno, "trailing characters after string"));
            }
            Ok(Value::Str(text))
        }
        '\'' => {
            let (text, rest) = parse_quoted(&s[1..], '\'', false, lineno)?;
            if !rest.trim().is_empty() {
                return Err(Error::parse_at(WHAT, lineno, "trailing characters after string"));
            }
            Ok(Value::Str(text))
        }
        '[' => parse_array(s, lineno),
        _ => parse_literal(s, lineno),
    }
}

fn parse_quoted<'a>(
    s: &'a str,
    quote: char,
    escapes: bool,
    lineno: usize,
) -> Result<(String, &'a str)> {
    let mut out = String::new();
    let mut iter = s.char_indices();
    while let Some((i, c)) = iter.next() {
        if c == quote {
            return Ok((out, &s[i + c.len_utf8()..]));
        }
        if escapes && c == '\\' {
            match iter.next() {
                Some((_, 'n')) => out.push('\n'),
                Some((_, 't')) => out.push('\t'),
                Some((_, 'r')) => out.push('\r'),
                Some((_, '"')) => out.push('"'),
                Some((_, '\'')) => out.push('\''),
                Some((_, '\\')) => out.push('\\'),
                Some((_, other)) => {
                    return Err(Error::parse_at(
                        WHAT,
                        lineno,
                        format!("unsupported escape '\\{other}'"),
                    ))
                }
                None => break,
            }
        } else {
            out.push(c);
        }
    }
    Err(Error::parse_at(WHAT, lineno, "unterminated string"))
}

fn parse_array(s: &str, lineno: usize) -> Result<Value> {
    if !s.ends_with(']') {
        return Err(Error::parse_at(WHAT, lineno, "unterminated array (must fit on one line)"));
    }
    let inner = s[1..s.len() - 1].trim();
    if inner.is_empty() {
        return Ok(Value::Array(Vec::new()));
    }
    let parts = split_top_level_commas(inner);
    let mut items = Vec::with_capacity(parts.len());
    for p in parts {
        items.push(parse_value(&p, lineno)?);
    }
    Ok(Value::Array(items))
}

fn parse_literal(s: &str, lineno: usize) -> Result<Value> {
    match s {
        "true" => return Ok(Value::Bool(true)),
        "false" => return Ok(Value::Bool(false)),
        _ => {}
    }
    let cleaned: String = s.chars().filter(|&c| c != '_').collect();
    if let Some(hex) = cleaned.strip_prefix("0x").or_else(|| cleaned.strip_prefix("0X")) {
        if let Ok(v) = i64::from_str_radix(hex, 16) {
            return Ok(Value::Int(v));
        }
    }
    if let Ok(v) = cleaned.parse::<i64>() {
        return Ok(Value::Int(v));
    }
    if let Ok(v) = cleaned.parse::<f64>() {
        return Ok(Value::Float(v));
    }
    Err(Error::parse_at(
        WHAT,
        lineno,
        format!("could not parse '{s}' (strings must be quoted)"),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::table_get;

    fn t(src: &str) -> Value {
        parse(src).unwrap()
    }

    #[test]
    fn scalars() {
        let doc = t("a = 1\nb = 1.5\nc = true\nd = \"hi\"\ne = 'raw\\n'\nf = 0x1F\ng = 1_000");
        let Value::Table(tab) = &doc else { panic!("root must be a table") };
        assert_eq!(table_get(tab, "a"), Some(&Value::Int(1)));
        assert_eq!(table_get(tab, "b"), Some(&Value::Float(1.5)));
        assert_eq!(table_get(tab, "c"), Some(&Value::Bool(true)));
        assert_eq!(table_get(tab, "d"), Some(&Value::Str("hi".into())));
        assert_eq!(table_get(tab, "e"), Some(&Value::Str("raw\\n".into()))); // literal: no escapes
        assert_eq!(table_get(tab, "f"), Some(&Value::Int(31)));
        assert_eq!(table_get(tab, "g"), Some(&Value::Int(1000)));
    }

    #[test]
    fn tables_and_dotted_keys() {
        let doc = t("[ui]\ntick = 250\n[ui.colors]\nfg = \"red\"\nother.deep.key = 7");
        let Value::Table(tab) = &doc else { panic!() };
        let Some(Value::Table(ui)) = table_get(tab, "ui") else { panic!() };
        assert_eq!(table_get(ui, "tick"), Some(&Value::Int(250)));
        let Some(Value::Table(colors)) = table_get(ui, "colors") else { panic!() };
        assert_eq!(table_get(colors, "fg"), Some(&Value::Str("red".into())));
    }

    #[test]
    fn arrays() {
        let doc = t("list = [\"a\", \"b\"]\nnums = [1, 2, 3,]\nempty = []");
        let Value::Table(tab) = &doc else { panic!() };
        let Some(Value::Array(items)) = table_get(tab, "list") else { panic!() };
        assert_eq!(items.len(), 2);
        let Some(Value::Array(nums)) = table_get(tab, "nums") else { panic!() };
        assert_eq!(nums.len(), 3);
    }

    #[test]
    fn comments_and_hashes_in_strings() {
        let doc = t("a = \"#nottag\" # comment\n# whole line\nb = 2 # x");
        let Value::Table(tab) = &doc else { panic!() };
        assert_eq!(table_get(tab, "a"), Some(&Value::Str("#nottag".into())));
        assert_eq!(table_get(tab, "b"), Some(&Value::Int(2)));
    }

    #[test]
    fn escapes_in_basic_strings() {
        let doc = t("s = \"a\\tb\\n\\\"c\\\"\"");
        let Value::Table(tab) = &doc else { panic!() };
        assert_eq!(table_get(tab, "s"), Some(&Value::Str("a\tb\n\"c\"".into())));
    }

    #[test]
    fn errors_point_at_lines() {
        let e = parse("ok = 1\nbad line").unwrap_err();
        assert!(e.to_string().contains("line 2"), "{e}");
        assert!(parse("a = ").is_err());
        assert!(parse("a = bareword").is_err());
        assert!(parse("a = \"unterminated").is_err());
        assert!(parse("a = [1, 2").is_err());
        assert!(parse("[table").is_err());
        assert!(parse("[[double]]").is_err());
        assert!(parse("a = 1\na = 2").is_err()); // duplicate
        assert!(parse("v = \"x\" trailing").is_err());
    }

    #[test]
    fn fragments_are_lenient() {
        assert_eq!(parse_fragment("slate-dark"), Value::Str("slate-dark".into()));
        assert_eq!(parse_fragment("250"), Value::Int(250));
        assert_eq!(parse_fragment("true"), Value::Bool(true));
        assert_eq!(parse_fragment("0.5"), Value::Float(0.5));
        assert_eq!(parse_fragment("\"quoted\""), Value::Str("quoted".into()));
        assert_eq!(
            parse_fragment("[cpu, memory, \"disk io\"]"),
            Value::Array(vec![
                Value::Str("cpu".into()),
                Value::Str("memory".into()),
                Value::Str("disk io".into())
            ])
        );
    }
}
