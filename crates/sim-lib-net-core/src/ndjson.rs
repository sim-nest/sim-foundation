//! Incremental newline-delimited JSON decoding.

use crate::NetError;
use crate::line::{DEFAULT_MAX_LINE_BYTES, LineDecoder};

/// Incremental newline-delimited JSON decoder.
///
/// Each complete line is returned verbatim as one record string (parsing is the
/// caller's job). A partial final line is buffered until a later push completes
/// it or [`flush`](NdjsonDecoder::flush) is called. Blank lines are skipped.
///
/// The default constructor uses [`DEFAULT_MAX_LINE_BYTES`] to guard against an
/// unbounded buffered line on an adversarial stream; exceeding it fails closed
/// with [`NetError::LineTooLong`]. [`unbounded`](NdjsonDecoder::unbounded) is
/// the explicit opt-out for trusted in-memory inputs.
#[derive(Debug)]
pub struct NdjsonDecoder {
    lines: LineDecoder,
    max_line_bytes: Option<usize>,
}

impl Default for NdjsonDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl NdjsonDecoder {
    /// Create a decoder using [`DEFAULT_MAX_LINE_BYTES`].
    pub fn new() -> Self {
        Self::with_max_line_bytes(DEFAULT_MAX_LINE_BYTES)
    }

    /// Create a decoder with no per-line size limit.
    ///
    /// Use this only for trusted, already-bounded inputs.
    pub fn unbounded() -> Self {
        Self {
            lines: LineDecoder::unbounded(),
            max_line_bytes: None,
        }
    }

    /// Create a decoder that rejects any line (or buffered partial) exceeding
    /// `max_line_bytes`.
    pub fn with_max_line_bytes(max_line_bytes: usize) -> Self {
        Self {
            lines: LineDecoder::with_max_line_bytes(max_line_bytes),
            max_line_bytes: Some(max_line_bytes),
        }
    }

    /// Append bytes and return completed JSON record strings, skipping blank
    /// lines. Fails closed if any completed line or the buffered remainder
    /// exceeds the configured limit.
    pub fn push(&mut self, bytes: &[u8]) -> Result<Vec<String>, NetError> {
        let mut records = Vec::new();
        for line in self.lines.push_checked(bytes)? {
            self.check_limit(line.len())?;
            if line.is_empty() {
                continue;
            }
            records.push(String::from_utf8_lossy(&line).into_owned());
        }
        Ok(records)
    }

    /// Take any buffered remainder as a final record. Returns `None` when the
    /// buffer is empty or holds only whitespace.
    pub fn flush(&mut self) -> Option<String> {
        self.flush_checked()
            .expect("ndjson line cap exceeded; use push to handle oversized input")
    }

    /// Take any buffered remainder as a final record, checking the configured
    /// line cap first.
    pub fn flush_checked(&mut self) -> Result<Option<String>, NetError> {
        let Some(line) = self.lines.flush_checked()? else {
            return Ok(None);
        };
        if line.is_empty() {
            return Ok(None);
        }
        self.check_limit(line.len())?;
        Ok(Some(String::from_utf8_lossy(&line).into_owned()))
    }

    fn check_limit(&self, len: usize) -> Result<(), NetError> {
        if let Some(limit) = self.max_line_bytes
            && len > limit
        {
            return Err(NetError::LineTooLong { max: limit });
        }
        Ok(())
    }
}
