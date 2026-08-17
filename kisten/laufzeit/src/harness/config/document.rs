//! Surgical single-member edits to a JSONC configuration source.
//!
//! `orchester.jsonc` belongs to the human who owns it: it may carry comments,
//! trailing commas and unquoted keys that no serializer round-trips.  So an
//! edit here is a *splice* of the original text.  Exactly one member's span is
//! rewritten and every other byte — every comment, every blank line, every
//! hand-chosen indent — is copied through untouched.
//!
//! Only as much JSON5 as is needed to walk to that span is understood.  The
//! loader remains the authority on whether a file is valid; this module either
//! finds its target or refuses.

use std::ops::Range;

use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub(crate) enum DocumentEditError {
    /// The source could not be walked far enough to place the edit.  The loader
    /// reports syntax problems with a position; an editor only has to decline.
    #[error("the configuration could not be read as a JSON object to edit")]
    Unreadable,
    /// A member along the path exists but does not hold an object, so
    /// descending into it would silently discard the value that is there.
    #[error("{path} already holds something other than an object")]
    NotAnObject { path: String },
}

/// Insert or replace the member at `path`, keeping the rest of `source` byte
/// for byte.
///
/// `render` receives the indentation the spliced value's own lines should
/// start from, so a multi-line value lines up with the block it lands in.
/// Missing intermediate objects along `path` are created.
pub(crate) fn upsert_member(
    source: &str,
    path: &[&str],
    render: impl Fn(&str) -> String,
) -> Result<String, DocumentEditError> {
    let root = root_object(source)?;
    upsert_at(source, root, path, 0, &render)
}

fn upsert_at(
    source: &str,
    open: usize,
    path: &[&str],
    depth: usize,
    render: &impl Fn(&str) -> String,
) -> Result<String, DocumentEditError> {
    let key = path.get(depth).ok_or(DocumentEditError::Unreadable)?;
    let last = depth + 1 == path.len();
    let (members, close) = object_members(source, open).ok_or(DocumentEditError::Unreadable)?;

    match members.iter().find(|member| member.key == *key) {
        Some(member) if last => {
            let indent = line_indent(source, member.key_start);
            Ok(splice(source, member.value.clone(), &render(&indent)))
        }
        Some(member) => {
            let nested = object_start(source, member.value.clone()).ok_or_else(|| {
                DocumentEditError::NotAnObject {
                    path: path[..=depth].join("."),
                }
            })?;
            upsert_at(source, nested, path, depth + 1, render)
        }
        None => {
            let indent = member_indent(source, open, members.last());
            let value = render_path(&path[depth + 1..], &indent, render);
            Ok(insert_member(
                source,
                open,
                close,
                members.last(),
                key,
                &value,
                &indent,
            ))
        }
    }
}

/// Wrap `value` in the objects the remaining path segments call for, so a
/// missing `model_providers` block is created around the entry that needed it.
fn render_path(rest: &[&str], indent: &str, render: &impl Fn(&str) -> String) -> String {
    match rest.split_first() {
        None => render(indent),
        Some((key, tail)) => {
            let inner = format!("{indent}  ");
            let nested = render_path(tail, &inner, render);
            format!(
                "{{\n{inner}{key}: {nested}\n{indent}}}",
                key = quote(key),
                nested = nested
            )
        }
    }
}

fn insert_member(
    source: &str,
    open: usize,
    close: usize,
    last: Option<&Member>,
    key: &str,
    value: &str,
    indent: &str,
) -> String {
    // A one-line object stays a one-line object: reflowing it would rewrite
    // text the human formatted deliberately.
    let multiline = source[open..close].contains('\n');
    let at = last.map(|member| member.end).unwrap_or(open + 1);
    let comma = if last.is_some_and(|member| !member.trailing_comma) {
        ","
    } else {
        ""
    };
    let entry = format!("{}: {value}", quote(key));
    let text = if multiline {
        format!("{comma}\n{indent}{entry}")
    } else if last.is_some() {
        format!("{comma} {entry}")
    } else {
        // An empty inline object has no member to sit beside, so it supplies
        // both of the spaces that would otherwise come from its neighbours.
        format!(" {entry} ")
    };
    format!("{}{text}{}", &source[..at], &source[at..])
}

/// Where a newly inserted member's line should start.
fn member_indent(source: &str, open: usize, last: Option<&Member>) -> String {
    match last {
        Some(member) => line_indent(source, member.key_start),
        None => format!("{}  ", line_indent(source, open)),
    }
}

fn splice(source: &str, span: Range<usize>, value: &str) -> String {
    format!("{}{value}{}", &source[..span.start], &source[span.end..])
}

/// The leading whitespace of the line `at` sits on.  A member sharing its line
/// with the opening brace reports none, which is what the inline branch of
/// [`insert_member`] then uses.
fn line_indent(source: &str, at: usize) -> String {
    let start = source[..at].rfind('\n').map_or(0, |index| index + 1);
    source[start..at]
        .chars()
        .take_while(|character| *character == ' ' || *character == '\t')
        .collect()
}

/// Quote a value as a JSON string, so a name carrying `"` or a control
/// character cannot break out and forge configuration structure.
pub(super) fn quote(value: &str) -> String {
    serde_json::Value::String(value.to_owned()).to_string()
}

fn root_object(source: &str) -> Result<usize, DocumentEditError> {
    let mut scanner = Scanner::new(source, 0);
    scanner.skip_trivia();
    match scanner.peek() {
        Some(b'{') => Ok(scanner.index),
        _ => Err(DocumentEditError::Unreadable),
    }
}

/// The index of the `{` opening the object `span` holds, if it holds one.
fn object_start(source: &str, span: Range<usize>) -> Option<usize> {
    let mut scanner = Scanner::new(source, span.start);
    scanner.skip_trivia();
    (scanner.index < span.end && scanner.peek() == Some(b'{')).then_some(scanner.index)
}

#[derive(Debug)]
struct Member {
    key: String,
    key_start: usize,
    value: Range<usize>,
    /// Index just past the member, including a comma that follows it.
    end: usize,
    trailing_comma: bool,
}

/// Every member of the object opening at `open`, plus the index of its `}`.
fn object_members(source: &str, open: usize) -> Option<(Vec<Member>, usize)> {
    let mut scanner = Scanner::new(source, open);
    if scanner.peek() != Some(b'{') {
        return None;
    }
    scanner.index += 1;
    let mut members = Vec::new();
    loop {
        scanner.skip_trivia();
        if scanner.peek()? == b'}' {
            return Some((members, scanner.index));
        }
        let (key, key_start) = scanner.read_key()?;
        scanner.skip_trivia();
        if scanner.peek()? != b':' {
            return None;
        }
        scanner.index += 1;
        scanner.skip_trivia();
        let value_start = scanner.index;
        scanner.skip_value()?;
        let value = value_start..scanner.index;
        scanner.skip_trivia();
        let trailing_comma = scanner.peek()? == b',';
        if trailing_comma {
            scanner.index += 1;
        }
        members.push(Member {
            key,
            key_start,
            end: if trailing_comma {
                scanner.index
            } else {
                value.end
            },
            value,
            trailing_comma,
        });
        if !trailing_comma {
            scanner.skip_trivia();
            return (scanner.peek()? == b'}').then_some((members, scanner.index));
        }
    }
}

struct Scanner<'a> {
    source: &'a str,
    index: usize,
}

impl<'a> Scanner<'a> {
    fn new(source: &'a str, index: usize) -> Self {
        Self { source, index }
    }

    fn peek(&self) -> Option<u8> {
        self.source.as_bytes().get(self.index).copied()
    }

    fn peek_next(&self) -> Option<u8> {
        self.source.as_bytes().get(self.index + 1).copied()
    }

    /// Advance past whitespace and both comment forms.  Comments are the reason
    /// this module exists, so every scan step has to step over them rather than
    /// mistake one for structure.
    fn skip_trivia(&mut self) {
        loop {
            match self.peek() {
                Some(byte) if byte.is_ascii_whitespace() => self.index += 1,
                Some(b'/') => match self.peek_next() {
                    Some(b'/') => {
                        while let Some(byte) = self.peek() {
                            self.index += 1;
                            if byte == b'\n' {
                                break;
                            }
                        }
                    }
                    Some(b'*') => {
                        self.index += 2;
                        while self.index < self.source.len() {
                            if self.peek() == Some(b'*') && self.peek_next() == Some(b'/') {
                                self.index += 2;
                                break;
                            }
                            self.index += 1;
                        }
                    }
                    _ => return,
                },
                _ => return,
            }
        }
    }

    /// Consume a quoted string and report the span of the literal, quotes
    /// included.
    fn read_string(&mut self) -> Option<Range<usize>> {
        let quote = self.peek()?;
        if quote != b'"' && quote != b'\'' {
            return None;
        }
        let start = self.index;
        self.index += 1;
        while let Some(byte) = self.peek() {
            match byte {
                b'\\' => self.index += 2,
                byte if byte == quote => {
                    self.index += 1;
                    return Some(start..self.index);
                }
                _ => self.index += 1,
            }
        }
        None
    }

    /// Read a member key, decoding escapes so that a key written the long way
    /// still matches the plain name being searched for.
    fn read_key(&mut self) -> Option<(String, usize)> {
        let start = self.index;
        match self.peek()? {
            b'"' | b'\'' => {
                let literal = self.read_string()?;
                let text = &self.source[literal];
                let key = serde_json::from_str::<String>(text).unwrap_or_else(|_| {
                    text.trim_matches(|character| character == '"' || character == '\'')
                        .to_owned()
                });
                Some((key, start))
            }
            _ => {
                while matches!(self.peek(), Some(byte) if byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$'))
                {
                    self.index += 1;
                }
                (self.index > start).then(|| (self.source[start..self.index].to_owned(), start))
            }
        }
    }

    fn skip_value(&mut self) -> Option<()> {
        match self.peek()? {
            b'{' | b'[' => self.skip_nested(),
            b'"' | b'\'' => self.read_string().map(|_| ()),
            _ => {
                // A scalar runs until the separator, the closing brace, or a
                // comment that was written right up against it.
                while let Some(byte) = self.peek() {
                    if matches!(byte, b',' | b'}' | b']') || byte.is_ascii_whitespace() {
                        break;
                    }
                    if byte == b'/' && matches!(self.peek_next(), Some(b'/' | b'*')) {
                        break;
                    }
                    self.index += 1;
                }
                Some(())
            }
        }
    }

    fn skip_nested(&mut self) -> Option<()> {
        self.index += 1;
        let mut depth = 1usize;
        while depth > 0 {
            self.skip_trivia();
            match self.peek()? {
                b'"' | b'\'' => {
                    self.read_string()?;
                }
                b'{' | b'[' => {
                    depth += 1;
                    self.index += 1;
                }
                b'}' | b']' => {
                    depth -= 1;
                    self.index += 1;
                }
                _ => self.index += 1,
            }
        }
        Some(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    /// Every edit has to survive the parser the loader actually uses, so each
    /// test checks the spliced text as JSON5 rather than only as characters.
    fn parsed(source: &str) -> Value {
        json5::from_str(source).expect("edited source stays parseable")
    }

    fn entry(indent: &str) -> String {
        format!("{{\n{indent}  \"base_url\": \"https://example.test\"\n{indent}}}")
    }

    #[test]
    fn replacing_an_entry_leaves_every_other_comment_and_neighbour_alone() {
        let source = "// mine, with notes\n\
             {\n  \
               // the provider I use\n  \
               \"model_provider\": \"relay\",\n  \
               \"model_providers\": {\n    \
                 // a relay that mirrors upstream model names\n    \
                 \"relay\": { \"base_url\": \"https://old.test\" },\n    \
                 \"direct\": { \"base_url\": \"https://direct.test\" }\n  \
               }\n\
             }\n";

        let edited = upsert_member(source, &["model_providers", "relay"], |indent| {
            entry(indent)
        })
        .expect("replace the relay entry");

        assert!(edited.contains("// mine, with notes"));
        assert!(edited.contains("// the provider I use"));
        assert!(edited.contains("// a relay that mirrors upstream model names"));
        assert!(edited.contains("https://example.test"));
        assert!(!edited.contains("https://old.test"));
        // The neighbour is a different member and must be copied through.
        assert!(edited.contains("https://direct.test"));
        assert_eq!(
            parsed(&edited)["model_providers"]["relay"]["base_url"],
            Value::String("https://example.test".into())
        );
    }

    #[test]
    fn a_new_entry_joins_the_existing_block_with_the_separator_it_needs() {
        let source = "{\n  \
               \"model_providers\": {\n    \
                 \"direct\": { \"base_url\": \"https://direct.test\" }\n  \
               }\n\
             }\n";

        let edited = upsert_member(source, &["model_providers", "relay"], |indent| {
            entry(indent)
        })
        .expect("insert the relay entry");

        let value = parsed(&edited);
        assert_eq!(
            value["model_providers"]["relay"]["base_url"],
            Value::String("https://example.test".into())
        );
        assert_eq!(
            value["model_providers"]["direct"]["base_url"],
            Value::String("https://direct.test".into())
        );
    }

    #[test]
    fn a_trailing_comma_is_not_doubled_into_a_syntax_error() {
        let source = "{\n  \
               \"model_providers\": {\n    \
                 \"direct\": { \"base_url\": \"https://direct.test\" },\n  \
               }\n\
             }\n";

        let edited = upsert_member(source, &["model_providers", "relay"], |indent| {
            entry(indent)
        })
        .expect("insert beside a trailing comma");

        assert!(!edited.contains(",,"));
        assert!(parsed(&edited)["model_providers"]["relay"].is_object());
    }

    #[test]
    fn a_missing_block_is_created_around_the_entry_that_needed_it() {
        let source = "// no providers yet\n{\n  \"version\": 1\n}\n";

        let edited = upsert_member(source, &["model_providers", "relay"], |indent| {
            entry(indent)
        })
        .expect("create the block");

        assert!(edited.contains("// no providers yet"));
        let value = parsed(&edited);
        assert_eq!(value["version"], Value::from(1));
        assert_eq!(
            value["model_providers"]["relay"]["base_url"],
            Value::String("https://example.test".into())
        );
    }

    #[test]
    fn an_empty_document_gains_the_whole_path() {
        let edited = upsert_member("{}\n", &["model_providers", "relay"], |indent| {
            entry(indent)
        })
        .expect("populate an empty object");

        assert!(parsed(&edited)["model_providers"]["relay"].is_object());
    }

    #[test]
    fn a_top_level_scalar_is_replaced_in_place() {
        let source =
            "{\n  \"model_provider\": \"direct\", // was the default\n  \"version\": 1\n}\n";

        let edited = upsert_member(source, &["model_provider"], |_| "\"relay\"".into())
            .expect("replace the active provider");

        assert!(edited.contains("// was the default"));
        let value = parsed(&edited);
        assert_eq!(value["model_provider"], Value::String("relay".into()));
        assert_eq!(value["version"], Value::from(1));
    }

    #[test]
    fn a_one_line_object_is_not_reflowed_into_a_block() {
        let source = "{ \"model_providers\": { \"direct\": 1 } }";

        let edited = upsert_member(source, &["model_providers", "relay"], |_| "2".into())
            .expect("insert inline");

        assert!(!edited.contains('\n'));
        assert_eq!(parsed(&edited)["model_providers"]["relay"], Value::from(2));
    }

    #[test]
    fn a_comment_between_the_entry_and_its_comma_does_not_shift_the_splice() {
        let source = "{\n  \"model_providers\": {\n    \"direct\": 1 /* keep */ ,\n    \"other\": 2\n  }\n}\n";

        let edited = upsert_member(source, &["model_providers", "direct"], |_| "3".into())
            .expect("replace beside a comment");

        assert!(edited.contains("/* keep */"));
        let value = parsed(&edited);
        assert_eq!(value["model_providers"]["direct"], Value::from(3));
        assert_eq!(value["model_providers"]["other"], Value::from(2));
    }

    #[test]
    fn an_escaped_key_still_matches_the_plain_name_instead_of_being_duplicated() {
        let source = "{\n  \"model_providers\": {\n    \"rel\\u0061y\": 1\n  }\n}\n";

        let edited = upsert_member(source, &["model_providers", "relay"], |_| "2".into())
            .expect("replace an escaped key");

        let providers = parsed(&edited)["model_providers"].clone();
        assert_eq!(providers.as_object().map(|block| block.len()), Some(1));
        assert_eq!(providers["relay"], Value::from(2));
    }

    #[test]
    fn a_path_through_a_non_object_is_refused_rather_than_overwritten() {
        let source = "{ \"model_providers\": \"see the other file\" }";

        let error = upsert_member(source, &["model_providers", "relay"], |_| "1".into())
            .expect_err("a string cannot be descended into");

        assert_eq!(
            error,
            DocumentEditError::NotAnObject {
                path: "model_providers".into()
            }
        );
    }

    #[test]
    fn a_source_that_is_not_an_object_is_refused() {
        for source in ["[1, 2]", "// only a comment\n", ""] {
            assert_eq!(
                upsert_member(source, &["model_providers", "relay"], |_| "1".into()),
                Err(DocumentEditError::Unreadable),
                "{source:?} has no object to edit"
            );
        }
    }
}
