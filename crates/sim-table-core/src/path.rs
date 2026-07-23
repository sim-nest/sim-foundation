//! Table path segments, references, and canonical resolution.
//!
//! [`TablePath`] is the canonical absolute identity: it stores only validated
//! table segments and formats with a leading `/` when it is rendered as a
//! reference. [`TablePathRef`] is the parsed user/reference form. It can be
//! absolute (`/a/b`) or relative (`../b`), can use `.` and `..` as traversal
//! components, percent-escapes non-plain bytes, and resolves against a base
//! [`TablePath`] without escaping above root.

use std::fmt;

/// Maximum number of path components a parsed reference can carry.
pub const MAX_TABLE_PATH_SEGMENTS: usize = 128;

/// Maximum byte length of one textual path reference.
pub const MAX_TABLE_PATH_TEXT_BYTES: usize = 4096;

/// Whether `name` is a legal single table path segment.
///
/// This is the exact predicate `sim-table-db` enforces in its `child_path`
/// check: a segment is illegal when it is empty, the relative `.`/`..` markers,
/// or contains a path separator (`/` or `\`). Everything else is legal.
pub fn is_legal_table_segment(name: &str) -> bool {
    !(name.is_empty() || name == "." || name == ".." || name.contains('/') || name.contains('\\'))
}

/// A validated, slash-joinable sequence of table path segments.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TablePath {
    segments: Vec<String>,
}

impl TablePath {
    /// Create an empty path.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create the root path.
    pub fn root() -> Self {
        Self::new()
    }

    /// Build a canonical path from validated segments.
    pub fn from_segments<I, S>(segments: I) -> Result<Self, TablePathError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut path = Self::new();
        for segment in segments {
            path.push(segment.as_ref())?;
        }
        Ok(path)
    }

    /// Parse an absolute textual path reference into a canonical path.
    pub fn parse_absolute(input: &str) -> Result<Self, TablePathRefError> {
        let reference = TablePathRef::parse(input)?;
        if !reference.is_absolute() {
            return Err(TablePathRefError::ExpectedAbsolute);
        }
        reference.resolve(&Self::root())
    }

    /// The accumulated segments, in order.
    pub fn segments(&self) -> &[String] {
        &self.segments
    }

    /// Whether this is the root path.
    pub fn is_root(&self) -> bool {
        self.segments.is_empty()
    }

    /// Append `segment`, validating it with [`is_legal_table_segment`].
    pub fn push(&mut self, segment: &str) -> Result<(), TablePathError> {
        if !is_legal_table_segment(segment) {
            return Err(TablePathError::IllegalSegment(segment.to_owned()));
        }
        if self.segments.len() == MAX_TABLE_PATH_SEGMENTS {
            return Err(TablePathError::TooManySegments {
                limit: MAX_TABLE_PATH_SEGMENTS,
            });
        }
        self.segments.push(segment.to_owned());
        Ok(())
    }

    /// Join the segments with `/`.
    pub fn join(&self) -> String {
        self.segments.join("/")
    }

    /// Format this canonical path as an absolute escaped path reference.
    pub fn to_absolute_reference(&self) -> String {
        TablePathRef::absolute(self).to_reference_string()
    }

    /// Resolve `reference` against this canonical path.
    pub fn resolve(&self, reference: &TablePathRef) -> Result<Self, TablePathRefError> {
        reference.resolve(self)
    }
}

impl fmt::Display for TablePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_absolute_reference())
    }
}

/// Why a [`TablePath`] operation failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TablePathError {
    /// The given segment did not satisfy [`is_legal_table_segment`].
    IllegalSegment(String),
    /// The path exceeds [`MAX_TABLE_PATH_SEGMENTS`].
    TooManySegments {
        /// The configured segment limit.
        limit: usize,
    },
}

/// One component in a textual table path reference.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TablePathRefPart {
    /// A validated path segment.
    Segment(String),
    /// The current path marker, `.`.
    Current,
    /// The parent path marker, `..`.
    Parent,
}

/// A parsed absolute or relative table path reference.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TablePathRef {
    absolute: bool,
    parts: Vec<TablePathRefPart>,
}

impl TablePathRef {
    /// Create a path reference from already separated components.
    pub fn new(absolute: bool, parts: Vec<TablePathRefPart>) -> Result<Self, TablePathRefError> {
        if parts.len() > MAX_TABLE_PATH_SEGMENTS {
            return Err(TablePathRefError::TooManySegments {
                limit: MAX_TABLE_PATH_SEGMENTS,
            });
        }
        for part in &parts {
            if let TablePathRefPart::Segment(segment) = part
                && !is_legal_table_segment(segment)
            {
                return Err(TablePathRefError::IllegalSegment(segment.clone()));
            }
        }
        Ok(Self { absolute, parts })
    }

    /// Return a relative reference to the current path.
    pub fn current() -> Self {
        Self {
            absolute: false,
            parts: vec![TablePathRefPart::Current],
        }
    }

    /// Return an absolute reference for `path`.
    pub fn absolute(path: &TablePath) -> Self {
        Self {
            absolute: true,
            parts: path
                .segments()
                .iter()
                .cloned()
                .map(TablePathRefPart::Segment)
                .collect(),
        }
    }

    /// Parse an absolute or relative textual path reference.
    pub fn parse(input: &str) -> Result<Self, TablePathRefError> {
        if input.is_empty() {
            return Err(TablePathRefError::EmptyReference);
        }
        if input.len() > MAX_TABLE_PATH_TEXT_BYTES {
            return Err(TablePathRefError::ReferenceTooLong {
                limit: MAX_TABLE_PATH_TEXT_BYTES,
            });
        }
        if input.as_bytes().contains(&b'\\') {
            return Err(TablePathRefError::AmbiguousSeparator('\\'));
        }

        let absolute = input.starts_with('/');
        let body = if absolute { &input[1..] } else { input };
        if body.is_empty() {
            return if absolute {
                Self::new(true, Vec::new())
            } else {
                Err(TablePathRefError::EmptyReference)
            };
        }

        let mut parts = Vec::new();
        for raw in body.split('/') {
            if raw.is_empty() {
                return Err(TablePathRefError::EmptySegment);
            }
            let segment = decode_segment(raw)?;
            let part = match segment.as_str() {
                "." => TablePathRefPart::Current,
                ".." => TablePathRefPart::Parent,
                _ if is_legal_table_segment(&segment) => TablePathRefPart::Segment(segment),
                _ => return Err(TablePathRefError::IllegalSegment(segment)),
            };
            parts.push(part);
        }
        Self::new(absolute, parts)
    }

    /// Whether this reference starts at root.
    pub fn is_absolute(&self) -> bool {
        self.absolute
    }

    /// The parsed reference components.
    pub fn parts(&self) -> &[TablePathRefPart] {
        &self.parts
    }

    /// Resolve this reference against `base`, returning a canonical path.
    pub fn resolve(&self, base: &TablePath) -> Result<TablePath, TablePathRefError> {
        let mut segments = if self.absolute {
            Vec::new()
        } else {
            base.segments.clone()
        };
        for part in &self.parts {
            match part {
                TablePathRefPart::Current => {}
                TablePathRefPart::Parent => {
                    if segments.pop().is_none() {
                        return Err(TablePathRefError::RootEscape);
                    }
                }
                TablePathRefPart::Segment(segment) => {
                    if segments.len() == MAX_TABLE_PATH_SEGMENTS {
                        return Err(TablePathRefError::TooManySegments {
                            limit: MAX_TABLE_PATH_SEGMENTS,
                        });
                    }
                    segments.push(segment.clone());
                }
            }
        }
        Ok(TablePath { segments })
    }

    /// Format this reference with percent-escaped segments and `/` separators.
    pub fn to_reference_string(&self) -> String {
        if self.absolute && self.parts.is_empty() {
            return "/".to_owned();
        }
        if !self.absolute && self.parts.is_empty() {
            return ".".to_owned();
        }

        let mut out = String::new();
        if self.absolute {
            out.push('/');
        }
        for (index, part) in self.parts.iter().enumerate() {
            if index > 0 {
                out.push('/');
            }
            match part {
                TablePathRefPart::Segment(segment) => out.push_str(&encode_segment(segment)),
                TablePathRefPart::Current => out.push('.'),
                TablePathRefPart::Parent => out.push_str(".."),
            }
        }
        out
    }
}

impl fmt::Display for TablePathRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_reference_string())
    }
}

/// Why parsing or resolving a [`TablePathRef`] failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TablePathRefError {
    /// The textual reference was empty.
    EmptyReference,
    /// A separator created an empty path component.
    EmptySegment,
    /// The textual reference used a separator other than `/`.
    AmbiguousSeparator(char),
    /// A percent escape was incomplete or contained a non-hex digit.
    BadEscape {
        /// The byte index within the raw segment.
        index: usize,
    },
    /// Percent escapes did not decode to UTF-8.
    InvalidUtf8Escape,
    /// The decoded component is not a legal table segment.
    IllegalSegment(String),
    /// The reference exceeds [`MAX_TABLE_PATH_SEGMENTS`].
    TooManySegments {
        /// The configured segment limit.
        limit: usize,
    },
    /// The textual reference exceeds [`MAX_TABLE_PATH_TEXT_BYTES`].
    ReferenceTooLong {
        /// The configured byte limit.
        limit: usize,
    },
    /// An absolute path was required.
    ExpectedAbsolute,
    /// Resolving `..` would move above root.
    RootEscape,
}

fn decode_segment(raw: &str) -> Result<String, TablePathRefError> {
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len() {
                return Err(TablePathRefError::BadEscape { index });
            }
            let high = hex_value(bytes[index + 1]).ok_or(TablePathRefError::BadEscape { index })?;
            let low = hex_value(bytes[index + 2]).ok_or(TablePathRefError::BadEscape { index })?;
            out.push((high << 4) | low);
            index += 3;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(out).map_err(|_| TablePathRefError::InvalidUtf8Escape)
}

fn encode_segment(segment: &str) -> String {
    let mut out = String::new();
    for byte in segment.bytes() {
        if is_unreserved_reference_byte(byte) {
            out.push(char::from(byte));
        } else {
            out.push('%');
            out.push(HEX[(byte >> 4) as usize] as char);
            out.push(HEX[(byte & 0x0F) as usize] as char);
        }
    }
    out
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn is_unreserved_reference_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~')
}

const HEX: &[u8; 16] = b"0123456789ABCDEF";
