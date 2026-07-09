//! Dotted/indexed JSON path extraction.
//!
//! Manifests address fields inside each agent's JSON line with paths like
//! `message.content[0].text`. This module resolves such paths against a
//! [`serde_json::Value`]. It is deliberately tiny — just object keys and array
//! indices — because that covers every real agent schema we normalize.

use serde_json::Value;

/// A single step in a path: an object key or an array index.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Segment {
    Key(String),
    Index(usize),
}

/// Parse `a.b[0].c` into `[Key(a), Key(b), Index(0), Key(c)]`.
fn parse(path: &str) -> Vec<Segment> {
    let mut segments = Vec::new();
    for part in path.split('.') {
        if part.is_empty() {
            continue;
        }
        // Split off any trailing `[i][j]…` index groups from the key.
        let mut rest = part;
        if let Some(bracket) = rest.find('[') {
            let key = &rest[..bracket];
            if !key.is_empty() {
                segments.push(Segment::Key(key.to_string()));
            }
            rest = &rest[bracket..];
            while let Some(close) = rest.find(']') {
                let idx = &rest[1..close];
                if let Ok(i) = idx.parse::<usize>() {
                    segments.push(Segment::Index(i));
                }
                rest = &rest[close + 1..];
                if !rest.starts_with('[') {
                    break;
                }
            }
        } else {
            segments.push(Segment::Key(part.to_string()));
        }
    }
    segments
}

/// Resolve `path` against `root`, returning the addressed value if present.
pub fn get<'a>(root: &'a Value, path: &str) -> Option<&'a Value> {
    let mut current = root;
    for seg in parse(path) {
        current = match seg {
            Segment::Key(k) => current.get(&k)?,
            Segment::Index(i) => current.get(i)?,
        };
    }
    Some(current)
}

/// Resolve `path` to an owned `String`. Accepts JSON strings directly; numbers
/// and booleans are stringified so manifests can point at scalar fields loosely.
pub fn get_string(root: &Value, path: &str) -> Option<String> {
    match get(root, path)? {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

/// Resolve `path` to a `u64` (0 if the field is absent or non-numeric-ish).
pub fn get_u64(root: &Value, path: &str) -> u64 {
    match get(root, path) {
        Some(Value::Number(n)) => n.as_u64().or_else(|| n.as_i64().map(|i| i.max(0) as u64)).unwrap_or(0),
        Some(Value::String(s)) => s.parse().unwrap_or(0),
        _ => 0,
    }
}

/// Resolve a manifest field value: a leading `=` marks a literal constant,
/// otherwise the value is treated as a JSON path resolved against `root`.
pub fn resolve_field(root: &Value, spec: &str) -> Option<String> {
    if let Some(literal) = spec.strip_prefix('=') {
        Some(literal.to_string())
    } else {
        get_string(root, spec)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn plain_key() {
        let v = json!({"result": "done"});
        assert_eq!(get_string(&v, "result").as_deref(), Some("done"));
    }

    #[test]
    fn nested_key_and_array_index() {
        let v = json!({"message": {"content": [{"text": "hi"}, {"text": "bye"}]}});
        assert_eq!(get_string(&v, "message.content[0].text").as_deref(), Some("hi"));
        assert_eq!(get_string(&v, "message.content[1].text").as_deref(), Some("bye"));
    }

    #[test]
    fn missing_path_is_none() {
        let v = json!({"a": {"b": 1}});
        assert!(get(&v, "a.c").is_none());
        assert!(get(&v, "a.b[0]").is_none());
        assert!(get_string(&v, "nope").is_none());
    }

    #[test]
    fn numbers_and_bools_stringify() {
        let v = json!({"n": 42, "flag": true});
        assert_eq!(get_string(&v, "n").as_deref(), Some("42"));
        assert_eq!(get_string(&v, "flag").as_deref(), Some("true"));
        assert_eq!(get_u64(&v, "n"), 42);
    }

    #[test]
    fn literal_vs_path() {
        let v = json!({"status": "running"});
        assert_eq!(resolve_field(&v, "=completed").as_deref(), Some("completed"));
        assert_eq!(resolve_field(&v, "status").as_deref(), Some("running"));
    }

    #[test]
    fn top_level_index() {
        let v = json!(["a", "b", "c"]);
        assert_eq!(get_string(&v, "[2]").as_deref(), Some("c"));
    }
}
