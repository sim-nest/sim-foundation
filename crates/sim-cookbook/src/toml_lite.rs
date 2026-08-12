//! A strict parser for the tiny TOML subset the cookbook manifests use.
//!
//! Supported, and nothing else:
//! - full-line comments (`# ...`) and trailing comments outside strings,
//! - top-level `key = value` where value is a quoted string, an integer, a
//!   bool, or an array of quoted strings (on one or several lines),
//! - `[[expect]]` array-of-tables, each holding `key = value` lines.
//!
//! Anything the parser does not understand is a hard error with a line number,
//! so a malformed manifest fails loudly instead of being silently misread.

/// A scalar or string-array value from a manifest line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TomlValue {
    /// A quoted string.
    Str(String),
    /// A signed integer.
    Int(i64),
    /// A boolean.
    Bool(bool),
    /// An array of strings.
    Array(Vec<String>),
}

impl TomlValue {
    /// The string value, or an error naming the actual type.
    pub fn as_str(&self) -> Result<&str, String> {
        match self {
            Self::Str(s) => Ok(s),
            other => Err(format!("expected string, found {}", other.type_name())),
        }
    }

    /// The integer value, or an error naming the actual type.
    pub fn as_int(&self) -> Result<i64, String> {
        match self {
            Self::Int(n) => Ok(*n),
            other => Err(format!("expected integer, found {}", other.type_name())),
        }
    }

    /// The string-array value, or an error naming the actual type.
    pub fn as_array(&self) -> Result<&[String], String> {
        match self {
            Self::Array(items) => Ok(items),
            other => Err(format!("expected array, found {}", other.type_name())),
        }
    }

    fn type_name(&self) -> &'static str {
        match self {
            Self::Str(_) => "string",
            Self::Int(_) => "integer",
            Self::Bool(_) => "bool",
            Self::Array(_) => "array",
        }
    }
}

/// A parsed manifest: top-level keys plus any `[[name]]` array-of-tables.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TomlDoc {
    /// Top-level `key = value` entries, in source order.
    pub top: Vec<(String, TomlValue)>,
    /// One `(name, entries)` per `[[name]]` table, in source order.
    pub tables: Vec<(String, Vec<(String, TomlValue)>)>,
}

impl TomlDoc {
    /// Look up a top-level key.
    pub fn get(&self, key: &str) -> Option<&TomlValue> {
        self.top.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    /// Every `[[name]]` table with the given name, in source order.
    pub fn tables_named(&self, name: &str) -> Vec<&[(String, TomlValue)]> {
        self.tables
            .iter()
            .filter(|(n, _)| n == name)
            .map(|(_, t)| t.as_slice())
            .collect()
    }

    /// Reject any top-level key not in `allowed` (strict schema check).
    pub fn reject_unknown_top(&self, allowed: &[&str]) -> Result<(), String> {
        for (k, _) in &self.top {
            if !allowed.contains(&k.as_str()) {
                return Err(format!("unknown key `{k}`"));
            }
        }
        Ok(())
    }

    /// Reject any `[[name]]` table whose name is not in `allowed`.
    pub fn reject_unknown_tables(&self, allowed: &[&str]) -> Result<(), String> {
        for (n, _) in &self.tables {
            if !allowed.contains(&n.as_str()) {
                return Err(format!("unknown table `[[{n}]]`"));
            }
        }
        Ok(())
    }
}

/// If `line` is a `[[name]]` array-of-table header, return `name`.
fn array_table_header(line: &str) -> Option<String> {
    let inner = line.strip_prefix("[[")?.strip_suffix("]]")?;
    let name = inner.trim();
    if !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        Some(name.to_string())
    } else {
        None
    }
}

/// Parse the full manifest text. Returns an error string with a line number on
/// the first construct it cannot accept.
pub fn parse(text: &str) -> Result<TomlDoc, String> {
    let mut doc = TomlDoc::default();
    // None = top level; Some(idx) = inside `tables[idx]`.
    let mut table: Option<usize> = None;
    let mut lines = text.lines().enumerate();

    while let Some((i, raw)) = lines.next() {
        let line_no = i + 1;
        let line = strip_trailing_comment(raw).trim();
        if line.is_empty() {
            continue;
        }
        if let Some(name) = array_table_header(line) {
            doc.tables.push((name, Vec::new()));
            table = Some(doc.tables.len() - 1);
            continue;
        }
        if line.starts_with('[') {
            return Err(format!("line {line_no}: unsupported table header `{line}`"));
        }
        let mut assignment = line.to_string();
        if assignment_starts_array(&assignment) && !array_is_complete(&assignment) {
            for (_, continuation) in lines.by_ref() {
                assignment.push('\n');
                assignment.push_str(strip_trailing_comment(continuation).trim());
                if array_is_complete(&assignment) {
                    break;
                }
            }
        }
        let (key, value) =
            parse_assignment(&assignment).map_err(|e| format!("line {line_no}: {e}"))?;
        match table {
            None => doc.top.push((key, value)),
            Some(idx) => doc.tables[idx].1.push((key, value)),
        }
    }
    Ok(doc)
}

fn assignment_starts_array(line: &str) -> bool {
    line.split_once('=')
        .is_some_and(|(_, value)| value.trim_start().starts_with('['))
}

/// Whether the array value in an assignment has a closing bracket outside a
/// quoted string. The subset has no nested arrays, but brackets inside strings
/// are valid data and must not terminate the value.
fn array_is_complete(line: &str) -> bool {
    let Some((_, value)) = line.split_once('=') else {
        return false;
    };
    let mut in_string = false;
    let mut escaped = false;
    for c in value.chars() {
        if in_string {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
        } else if c == '"' {
            in_string = true;
        } else if c == ']' {
            return true;
        }
    }
    false
}

fn parse_assignment(line: &str) -> Result<(String, TomlValue), String> {
    let eq = line.find('=').ok_or("expected `key = value`")?;
    let key = line[..eq].trim();
    if key.is_empty()
        || !key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(format!("invalid key `{key}`"));
    }
    let value = parse_value(line[eq + 1..].trim())?;
    Ok((key.to_string(), value))
}

fn parse_value(s: &str) -> Result<TomlValue, String> {
    if s.starts_with('"') {
        return Ok(TomlValue::Str(parse_string(s)?));
    }
    if s.starts_with('[') {
        return Ok(TomlValue::Array(parse_string_array(s)?));
    }
    if s == "true" {
        return Ok(TomlValue::Bool(true));
    }
    if s == "false" {
        return Ok(TomlValue::Bool(false));
    }
    if let Ok(n) = s.parse::<i64>() {
        return Ok(TomlValue::Int(n));
    }
    Err(format!("unrecognized value `{s}`"))
}

/// Parse one double-quoted string that occupies the whole of `s`.
fn parse_string(s: &str) -> Result<String, String> {
    let (value, rest) = take_string(s)?;
    if !rest.trim().is_empty() {
        return Err(format!("trailing text after string: `{}`", rest.trim()));
    }
    Ok(value)
}

/// Parse a leading double-quoted string, returning it and the remainder.
fn take_string(s: &str) -> Result<(String, &str), String> {
    let bytes = s.as_bytes();
    if bytes.first() != Some(&b'"') {
        return Err("expected `\"`".to_string());
    }
    let mut out = String::new();
    let mut chars = s.char_indices().skip(1);
    while let Some((idx, c)) = chars.next() {
        match c {
            '"' => return Ok((out, &s[idx + 1..])),
            '\\' => match chars.next() {
                Some((_, 'n')) => out.push('\n'),
                Some((_, 't')) => out.push('\t'),
                Some((_, '"')) => out.push('"'),
                Some((_, '\\')) => out.push('\\'),
                Some((_, other)) => out.push(other),
                None => return Err("unterminated escape".to_string()),
            },
            other => out.push(other),
        }
    }
    Err("unterminated string".to_string())
}

fn parse_string_array(s: &str) -> Result<Vec<String>, String> {
    let s = s.strip_prefix('[').ok_or("expected `[`")?;
    let inner = s
        .strip_suffix(']')
        .ok_or("unterminated array (missing `]`)")?;
    let mut items = Vec::new();
    let mut rest = inner.trim();
    while !rest.is_empty() {
        if !rest.starts_with('"') {
            return Err(format!(
                "array elements must be quoted strings, found `{rest}`"
            ));
        }
        let (value, after) = take_string(rest)?;
        items.push(value);
        rest = after.trim_start();
        if let Some(stripped) = rest.strip_prefix(',') {
            rest = stripped.trim_start();
        } else if !rest.is_empty() {
            return Err(format!(
                "expected `,` between array elements, found `{rest}`"
            ));
        }
    }
    Ok(items)
}

/// Cut a line at the first `#` that is not inside a double-quoted string.
fn strip_trailing_comment(line: &str) -> &str {
    let mut in_string = false;
    let mut escaped = false;
    for (idx, c) in line.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
        } else if c == '"' {
            in_string = true;
        } else if c == '#' {
            return &line[..idx];
        }
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_scalars_and_arrays() {
        let doc = parse(
            r#"
            # a comment
            id = "add"        # trailing comment
            order = 100
            flag = true
            tags = ["a", "b"]
            empty = []
            "#,
        )
        .unwrap();
        assert_eq!(doc.get("id").unwrap().as_str().unwrap(), "add");
        assert_eq!(doc.get("order").unwrap().as_int().unwrap(), 100);
        assert_eq!(doc.get("flag").unwrap(), &TomlValue::Bool(true));
        assert_eq!(doc.get("tags").unwrap().as_array().unwrap(), &["a", "b"]);
        assert!(doc.get("empty").unwrap().as_array().unwrap().is_empty());
    }

    #[test]
    fn parses_multiline_string_arrays_with_comments_and_trailing_comma() {
        let doc = parse(
            r#"
            chapters = [
              "01-basics",
              "02-techniques", # retained ordering
              "bracket-]--inside-string",
            ]
            title = "Book"
            "#,
        )
        .unwrap();
        assert_eq!(
            doc.get("chapters").unwrap().as_array().unwrap(),
            ["01-basics", "02-techniques", "bracket-]--inside-string"]
        );
        assert_eq!(doc.get("title").unwrap().as_str().unwrap(), "Book");
    }

    #[test]
    fn parses_named_tables() {
        let doc = parse("title = \"x\"\n[[expect]]\nform = 0\nresult = \"3\"\n").unwrap();
        let expect = doc.tables_named("expect");
        assert_eq!(expect.len(), 1);
        assert_eq!(expect[0][0].0, "form");
        assert_eq!(expect[0][1].1.as_str().unwrap(), "3");
        assert!(doc.reject_unknown_tables(&["expect"]).is_ok());
        assert!(doc.reject_unknown_tables(&[]).is_err());
    }

    #[test]
    fn parses_multiple_named_tables() {
        let doc =
            parse("[[hide]]\nrecipe = \"a\"\n[[reorder]]\nrecipe = \"b\"\norder = 1\n").unwrap();
        assert_eq!(doc.tables_named("hide").len(), 1);
        assert_eq!(doc.tables_named("reorder").len(), 1);
    }

    #[test]
    fn hash_inside_string_is_kept() {
        let doc = parse("title = \"a # b\"\n").unwrap();
        assert_eq!(doc.get("title").unwrap().as_str().unwrap(), "a # b");
    }

    #[test]
    fn rejects_unterminated_string() {
        assert!(parse("id = \"oops\n").is_err());
    }

    #[test]
    fn rejects_unknown_table() {
        let err = parse("[server]\n").unwrap_err();
        assert!(err.contains("unsupported table header"));
    }

    #[test]
    fn reject_unknown_top_flags_extra_keys() {
        let doc = parse("id = \"x\"\nbogus = 1\n").unwrap();
        assert!(doc.reject_unknown_top(&["id"]).is_err());
        assert!(doc.reject_unknown_top(&["id", "bogus"]).is_ok());
    }
}
