//! Incremental newline-delimited JSON decoding.

use crate::NetError;
use crate::line::LineDecoder;

/// Incremental newline-delimited JSON decoder.
///
/// Each complete line is returned verbatim as one record string (parsing is the
/// caller's job). A partial final line is buffered until a later push completes
/// it or [`flush`](NdjsonDecoder::flush) is called. Blank lines are skipped.
///
/// An optional per-line size limit guards against an unbounded buffered line on
/// an adversarial stream; exceeding it fails closed with
/// [`NetError::OversizeBody`].
#[derive(Debug, Default)]
pub struct NdjsonDecoder {
    lines: LineDecoder,
    max_line_bytes: Option<usize>,
}

impl NdjsonDecoder {
    /// Create a decoder with no per-line size limit.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a decoder that rejects any line (or buffered partial) exceeding
    /// `max_line_bytes`.
    pub fn with_max_line_bytes(max_line_bytes: usize) -> Self {
        Self {
            lines: LineDecoder::new(),
            max_line_bytes: Some(max_line_bytes),
        }
    }

    /// Append bytes and return completed JSON record strings, skipping blank
    /// lines. Fails closed if any completed line or the buffered remainder
    /// exceeds the configured limit.
    pub fn push(&mut self, bytes: &[u8]) -> Result<Vec<String>, NetError> {
        let mut records = Vec::new();
        for line in self.lines.push(bytes) {
            self.check_limit(line.len())?;
            if line.is_empty() {
                continue;
            }
            records.push(String::from_utf8_lossy(&line).into_owned());
        }
        // A still-buffered partial line can also blow the limit before its
        // newline ever arrives.
        self.check_limit(self.lines.buffered_len())?;
        Ok(records)
    }

    /// Take any buffered remainder as a final record. Returns `None` when the
    /// buffer is empty or holds only whitespace.
    pub fn flush(&mut self) -> Option<String> {
        let line = self.lines.flush()?;
        if line.is_empty() {
            return None;
        }
        Some(String::from_utf8_lossy(&line).into_owned())
    }

    fn check_limit(&self, len: usize) -> Result<(), NetError> {
        if let Some(limit) = self.max_line_bytes
            && len > limit
        {
            return Err(NetError::OversizeBody(limit));
        }
        Ok(())
    }
}
