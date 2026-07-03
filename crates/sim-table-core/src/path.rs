//! Table path segments and the shared legal-segment predicate.

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

    /// The accumulated segments, in order.
    pub fn segments(&self) -> &[String] {
        &self.segments
    }

    /// Append `segment`, validating it with [`is_legal_table_segment`].
    pub fn push(&mut self, segment: &str) -> Result<(), TablePathError> {
        if !is_legal_table_segment(segment) {
            return Err(TablePathError::IllegalSegment(segment.to_owned()));
        }
        self.segments.push(segment.to_owned());
        Ok(())
    }

    /// Join the segments with `/`.
    pub fn join(&self) -> String {
        self.segments.join("/")
    }
}

/// Why a [`TablePath`] operation failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TablePathError {
    /// The given segment did not satisfy [`is_legal_table_segment`].
    IllegalSegment(String),
}
