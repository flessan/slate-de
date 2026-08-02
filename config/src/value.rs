//! The value model of the TOML subset.

/// A parsed value. Tables preserve insertion order (stable round-trips).
#[derive(Clone, Debug, PartialEq)]
#[derive(Default)]
#[derive(Default)]
pub enum Value {
    Str(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    Array(Vec<Value>),
    Table(Vec<(String, Value)>),
}

impl Value {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_int(&self) -> Option<i64> {
        match self {
            Value::Int(i) => Some(*i),
            _ => None,
        }
    }

    pub fn as_float(&self) -> Option<f64> {
        match self {
            Value::Float(f) => Some(*f),
            Value::Int(i) => Some(*i as f64),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_table(&self) -> Option<&[(String, Value)]> {
        match self {
            Value::Table(t) => Some(t),
            _ => None,
        }
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Str(_) => "string",
            Value::Int(_) => "integer",
            Value::Float(_) => "float",
            Value::Bool(_) => "boolean",
            Value::Array(_) => "array",
            Value::Table(_) => "table",
        }
    }

    /// Display form (strings unquoted) — for `config get` output.
    pub fn display(&self) -> String {
        match self {
            Value::Str(s) => s.clone(),
            Value::Int(i) => i.to_string(),
            Value::Float(f) => f.to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Array(items) => {
                let inner: Vec<String> = items.iter().map(Value::display).collect();
                format!("[{}]", inner.join(", "))
            }
            Value::Table(t) => format!("<table: {} keys>", t.len()),
        }
    }

    /// TOML serialization for scalars/arrays (tables handled by writer).
    pub fn to_toml_value(&self) -> String {
        match self {
            Value::Str(s) => format!("\"{}\"", escape_basic(s)),
            Value::Int(i) => i.to_string(),
            Value::Float(f) => {
                // Always include a decimal point for floats
                if f.fract() == 0.0 && f.is_finite() {
                    format!("{f:.1}")
                } else {
                    f.to_string()
                }
            }
            Value::Bool(b) => b.to_string(),
            Value::Array(items) => {
                let inner: Vec<String> = items.iter().map(Value::to_toml_value).collect();
                format!("[{}]", inner.join(", "))
            }
            Value::Table(_) => "<table>".to_string(),
        }
    }
}

pub fn escape_basic(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            _ => out.push(c),
        }
    }
    out
}

/// Map helpers over the ordered table vector.
pub fn table_get<'a>(table: &'a [(String, Value)], key: &str) -> Option<&'a Value> {
    table.iter().find(|(k, _)| k == key).map(|(_, v)| v)
}

pub fn table_get_mut<'a>(table: &'a mut [(String, Value)], key: &str) -> Option<&'a mut Value> {
    table.iter_mut().find(|(k, _)| k == key).map(|(_, v)| v)
}

pub fn table_insert(table: &mut Vec<(String, Value)>, key: String, value: Value) {
    if let Some(slot) = table_get_mut(table, &key) {
        *slot = value;
    } else {
        table.push((key, value));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn displays_and_serializes() {
        assert_eq!(Value::Str("dark".into()).display(), "dark");
        assert_eq!(Value::Int(3).to_toml_value(), "3");
        assert_eq!(Value::Float(2.0).to_toml_value(), "2.0");
        assert_eq!(Value::Str("a\"b".into()).to_toml_value(), "\"a\\\"b\"");
        assert_eq!(
            Value::Array(vec![Value::Str("x".into()), Value::Int(1)]).to_toml_value(),
            "[\"x\", 1]"
        );
    }
}
